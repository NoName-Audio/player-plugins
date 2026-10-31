use player_plugin_sdk::{Host, NativePlugin, Result};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Default)]
pub struct Plugin {
    config: Value,
    has_password: bool,
    error: String,
    notice: String,
    playback: Value,
    query: String,
    replacement: Vec<Value>,
    page: usize,
    generation: u64,
    searches: BTreeMap<String, Vec<Value>>,
    results: BTreeMap<String, Value>,
    transfers: BTreeMap<String, Value>,
    pending_play: BTreeSet<String>,
    pending_save: BTreeMap<String, Option<i64>>,
    tokens: BTreeMap<String, Value>,
    busy: BTreeSet<String>,
    covers: BTreeMap<String, Value>,
    cover_pending: BTreeSet<String>,
    cover_queue: VecDeque<Value>,
    cover_workers: usize,
}
fn text(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or("").to_owned()
}
fn number(v: &Value, key: &str) -> f64 {
    v[key].as_f64().unwrap_or(0.)
}
fn key(v: &Value) -> String {
    json!([v["username"], v["filename"]]).to_string()
}
fn failed(t: &Value) -> bool {
    matches!(
        t["state"].as_str(),
        Some("failed" | "timed_out" | "cancelled")
    )
}
impl Plugin {
    fn resource(&self, r: &Value) -> Value {
        json!({"pluginId":"player.soulseek","resourceId":key(r),"title":r["basename"],"artist":r["username"],"album":"","durationSeconds":r["duration"],"coverHash":r["coverHash"]})
    }
    fn persist(&mut self, host: &dyn Host) -> Result {
        let ids: BTreeSet<_> = self
            .pending_play
            .iter()
            .chain(self.pending_save.keys())
            .cloned()
            .collect();
        let intents:Vec<_>=ids.iter().filter_map(|id|self.results.get(id).map(|r|json!({"result":r,"play":self.pending_play.contains(id),"save":self.pending_save.contains_key(id),"trackId":self.pending_save.get(id).copied().flatten(),"generation":self.generation}))).collect();
        let latest = host.call("settings.get", json!({}))?;
        let mut values = latest["values"].clone();
        values["transferIntents"] = json!(intents);
        self.config = host.call(
            "settings.update",
            json!({"revision":latest["revision"],"values":values}),
        )?;
        Ok(Value::Null)
    }
    fn row(&self, r: &Value) -> Value {
        let id = key(r);
        let t = self.transfers.get(&id).cloned().unwrap_or(Value::Null);
        let mut format = vec![];
        for (field, suffix) in [
            ("extension", ""),
            ("bitrate", " kbps"),
            ("sampleRate", " Hz"),
            ("bitDepth", "-bit"),
        ] {
            if let Some(v) = r
                .get(field)
                .filter(|v| !v.is_null() && **v != json!(0) && **v != json!(""))
            {
                format.push(format!(
                    "{}{}",
                    v.as_str()
                        .map(String::from)
                        .unwrap_or_else(|| v.to_string()),
                    suffix
                ));
            }
        }
        let subtitle = format!(
            "{} · {} KiB · {}{}{}{}",
            text(r, "username"),
            (number(r, "size") / 1024.).round(),
            format.join(" · "),
            if number(r, "duration") > 0. {
                format!(" · {} s", number(r, "duration").round())
            } else {
                String::new()
            },
            if number(r, "peerSpeed") > 0. {
                format!(" · {} KiB/s", (number(r, "peerSpeed") / 1024.).round())
            } else {
                String::new()
            },
            if number(r, "freeUploadSlots") > 0. {
                format!(" · {} slots", number(r, "freeUploadSlots"))
            } else {
                String::new()
            }
        );
        let mut status = text(&t, "state");
        if let Some(bytes) = t["bytesDownloaded"].as_f64() {
            status += &format!(" · {} KiB", (bytes / 1024.).round());
        }
        if let Some(position) = t["queuePosition"].as_u64() {
            status += &format!(" · queue {position}");
        }
        let playing = self.playback["playing"] == true
            && self.playback["resource"]["pluginId"] == "player.soulseek"
            && self.playback["resource"]["resourceId"] == id;
        json!({"id":id,"title":r["basename"],"subtitle":subtitle,"coverHash":self.results.get(&id).map(|row|row["coverHash"].clone()).unwrap_or_else(||r["coverHash"].clone()),"status":status,"actions":[
            {"id":"play","label":if self.pending_play.contains(&id){"Cancel preview"}else if playing{"Stop preview"}else{"Preview"}},
            {"id":"save","label":if self.busy.contains(&id){"Saving…"}else{"Save to library"},"disabled":self.busy.contains(&id)}]})
    }
    fn search(&mut self, query: &str, host: &dyn Host) -> Result<Vec<Value>> {
        if host.call("soulseek.status", json!({}))?["configured"] != true {
            return Err("Add your username and password in Soulseek settings first.".into());
        }
        let result = host
            .call("soulseek.search", json!({"query":query}))?
            .as_array()
            .cloned()
            .ok_or("Invalid Soulseek results")?;
        for r in &result {
            self.results.insert(key(r), r.clone());
        }
        Ok(result)
    }
    fn covers(&mut self, rows: &[Value], host: &dyn Host) {
        for r in rows {
            if r["coverFilename"].is_null() || number(r, "coverSize") <= 0. {
                continue;
            }
            let id = json!([r["username"], r["coverFilename"]]).to_string();
            if !self.covers.contains_key(&id) && self.cover_pending.insert(id) {
                self.cover_queue.push_back(r.clone());
            }
        }
        while self.cover_workers < 2 {
            let Some(r) = self.cover_queue.pop_front() else {
                break;
            };
            let id = json!([r["username"], r["coverFilename"]]).to_string();
            let task=host.call("host.background",json!({"topic":"soulseek-cover","id":id,"method":"soulseek.cover","args":{"request":{"username":r["username"],"coverFilename":r["coverFilename"],"coverSize":r["coverSize"]}}}));
            if task.is_ok() {
                self.cover_workers += 1;
            } else {
                self.cover_pending.remove(&id);
                self.covers.insert(id, Value::Null);
            }
        }
    }
    fn prepare(&mut self, r: &Value, host: &dyn Host) -> Result {
        let id = key(r);
        if let Some(token) = self.tokens.get(&id) {
            return Ok(token.clone());
        }
        let token = host.call(
            "soulseek.prepare",
            json!({"username":r["username"],"filename":r["filename"],"resource":self.resource(r)}),
        )?;
        self.tokens.insert(id, token.clone());
        Ok(token)
    }
    fn ready(r: &Value, t: &Value) -> bool {
        if t["localPath"].as_str().is_none() {
            return false;
        }
        if t["state"] == "completed" {
            return true;
        }
        let threshold = if number(r, "bitrate") > 0. {
            (number(r, "bitrate") * 1000. / 8. * 18.).max(384. * 1024.)
        } else {
            1_500_000.
        };
        number(t, "bytesDownloaded") >= number(r, "size").min(threshold)
    }
    fn play(&mut self, r: &Value, host: &dyn Host) -> Result {
        let token = self.prepare(r, host)?;
        host.call(
            "host.play",
            json!({"sourceToken":token,"resource":self.resource(r)}),
        )
    }
    fn save(&mut self, r: &Value, track_id: Option<i64>, host: &dyn Host) -> Result {
        let id = key(r);
        if !self.busy.insert(id.clone()) {
            return Ok(Value::Null);
        }
        host.invalidate();
        let saved = (|| {
            let token = self.prepare(r, host)?;
            let track = host.call(
                if track_id.is_some() {
                    "library.replace"
                } else {
                    "library.import"
                },
                json!({"sourceToken":token,"trackId":track_id,"coverHash":r["coverHash"]}),
            )?;
            if track.is_null() {
                return Err("The downloaded file could not be indexed".into());
            }
            host.call("host.adopt", json!({"resourceId":id,"track":track}))?;
            self.notice = if track_id.is_some() {
                "Track replaced."
            } else {
                "Saved to your library."
            }
            .into();
            Ok(Value::Null)
        })();
        self.busy.remove(&id);
        self.pending_save.remove(&id);
        let persisted = self.persist(host);
        host.invalidate();
        saved.and(persisted)
    }
    fn start(&mut self, r: &Value, host: &dyn Host) -> Result {
        let id = key(r);
        if self.transfers.get(&id).is_some_and(|t| !failed(t)) {
            return Ok(Value::Null);
        }
        self.tokens.remove(&id);
        self.transfers
            .insert(id.clone(), json!({"state":"starting","bytesDownloaded":0}));
        match host.call("soulseek.preview",json!({"request":{"username":r["username"],"filename":r["filename"],"size":r["size"],"coverFilename":r["coverFilename"],"coverSize":r["coverSize"]}})) {
            Ok(v)=>Ok(v),Err(e)=>{self.transfers.insert(id.clone(),json!({"state":"failed","error":e}));self.pending_play.remove(&id);self.pending_save.remove(&id);let _=self.persist(host);Err(e)}
        }
    }
    fn transfer(&mut self, t: Value, host: &dyn Host) -> Result {
        let id = key(&t);
        self.transfers.insert(id.clone(), t.clone());
        host.invalidate();
        let Some(r) = self.results.get(&id).cloned() else {
            return Ok(Value::Null);
        };
        if failed(&t) {
            self.pending_play.remove(&id);
            self.pending_save.remove(&id);
            self.error = t["error"].as_str().unwrap_or("Transfer failed").into();
            return self.persist(host);
        }
        if self.pending_play.contains(&id) && Self::ready(&r, &t) {
            self.pending_play.remove(&id);
            self.play(&r, host)?;
            self.persist(host)?;
        }
        if t["state"] == "completed" && self.pending_save.contains_key(&id) {
            let track = self.pending_save[&id];
            self.save(&r, track, host)?;
        }
        Ok(Value::Null)
    }
    fn render(&mut self, args: Value, host: &dyn Host) -> Result {
        let view = text(&args, "viewId");
        let mut messages = vec![];
        if !self.error.is_empty() {
            messages.push(json!({"type":"text","tone":"error","text":self.error}));
        }
        if !self.notice.is_empty() {
            messages.push(json!({"type":"text","text":self.notice}));
        }
        if view == "connection" {
            let mut children = vec![
                json!({"type":"text","text":"Use your Soulseek account. Downloaded music is imported into your library using artist and album tags."}),
                json!({"type":"input","key":"username","label":"Username","value":self.config["values"]["username"].as_str().unwrap_or("")}),
                json!({"type":"input","key":"password","label":if self.has_password{"New password (leave blank to keep saved)"}else{"Password"},"inputType":"password","value":""}),
                json!({"type":"button","label":"Save connection","action":"save-settings"}),
            ];
            children.extend(messages);
            return Ok(json!({"type":"group","title":"Soulseek connection","children":children}));
        }
        if view == "buffer" {
            let count = self
                .transfers
                .values()
                .filter(|t| {
                    matches!(
                        t["state"].as_str(),
                        Some("starting" | "queued_local" | "queued_remote" | "progress")
                    )
                })
                .count();
            return Ok(
                json!({"type":"text","text":if count==0{String::new()}else{format!("Soulseek · {count} active transfer{}",if count==1{""}else{"s"})}}),
            );
        }
        let track = &args["context"]["track"];
        let rows: Vec<_> = self
            .replacement
            .iter()
            .skip(self.page * 120)
            .take(120)
            .cloned()
            .collect();
        self.covers(&rows, host);
        let items: Vec<_> = rows
            .iter()
            .map(|r| {
                let mut row = self.row(self.results.get(&key(r)).unwrap_or(r));
                if !track.is_null() {
                    row["actions"]
                        .as_array_mut()
                        .unwrap()
                        .push(json!({"id":"replace","label":"Replace track"}));
                }
                row
            })
            .collect();
        let query = if !self.query.is_empty() {
            self.query.clone()
        } else {
            [text(track, "artist"), text(track, "title")]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" ")
        };
        let mut children = vec![
            json!({"type":"input","key":"query","label":"Search query","value":query}),
            json!({"type":"button","label":"Search","action":"replacement-search"}),
        ];
        children.extend(messages);
        children.push(json!({"type":"results","items":items}));
        if self.page > 0 {
            children.push(
                json!({"type":"button","label":"Previous results","action":"replacement-previous"}),
            );
        }
        if (self.page + 1) * 120 < self.replacement.len() {
            children
                .push(json!({"type":"button","label":"Next results","action":"replacement-next"}));
        }
        Ok(
            json!({"type":"group","title":if track.is_null(){"Soulseek".into()}else{format!("Replace {}",track["title"].as_str().or(track["path"].as_str()).unwrap_or("track"))},"children":children}),
        )
    }
    fn action(&mut self, args: Value, host: &dyn Host) -> Result {
        let action = text(&args, "action");
        let fields = &args["fields"];
        let payload = &args["payload"];
        match action.as_str() {
            "save-settings" => {
                if let Some(password) = fields["password"].as_str().filter(|p| !p.is_empty()) {
                    host.call("secrets.set", json!({"key":"password","value":password}))?;
                }
                let latest = host.call("settings.get", json!({}))?;
                let mut values = latest["values"].clone();
                values["username"] = json!(text(fields, "username").trim());
                self.config = host.call(
                    "settings.update",
                    json!({"revision":latest["revision"],"values":values}),
                )?;
                self.has_password = host.call("secrets.has", json!({"key":"password"}))? == true;
                self.notice = "Connection saved.".into();
                return Ok(Value::Null);
            }
            "find-replacement" => {
                self.query.clear();
                self.replacement.clear();
                self.page = 0;
                return host.call(
                    "host.navigate",
                    json!({"pageId":"replacement","context":{"track":payload["track"]}}),
                );
            }
            "replacement-search" => {
                self.query = text(fields, "query").trim().into();
                self.page = 0;
                if !self.query.is_empty() {
                    self.replacement = self.search(&self.query.clone(), host)?;
                }
                return Ok(Value::Null);
            }
            "replacement-next" => {
                self.page =
                    (self.page + 1).min(self.replacement.len().div_ceil(120).saturating_sub(1));
                return Ok(Value::Null);
            }
            "replacement-previous" => {
                self.page = self.page.saturating_sub(1);
                return Ok(Value::Null);
            }
            _ => {}
        }
        let id = text(payload, "resourceId");
        let r = self
            .results
            .get(&id)
            .cloned()
            .ok_or("Search again to restore this result")?;
        if action == "play" {
            if self.playback["playing"] == true
                && self.playback["resource"]["pluginId"] == "player.soulseek"
                && self.playback["resource"]["resourceId"] == id
            {
                return host.call("host.transport", json!({"action":"stop"}));
            }
            if self.pending_play.remove(&id) {
                self.transfers
                    .insert(id.clone(), json!({"state":"cancelled"}));
                self.tokens.remove(&id);
                self.persist(host)?;
                return host.call(
                    "soulseek.cancelPreview",
                    json!({"request":{"username":r["username"],"filename":r["filename"]}}),
                );
            }
            if self.transfers.get(&id).is_some_and(|t| Self::ready(&r, t)) {
                return self.play(&r, host);
            }
            self.pending_play.clear();
            self.pending_play.insert(id);
            self.persist(host)?;
            self.start(&r, host)
        } else if action == "save" || action == "replace" {
            let track_id = if action == "replace" {
                Some(
                    args["context"]["track"]["id"]
                        .as_i64()
                        .ok_or("Choose the track to replace first")?,
                )
            } else {
                None
            };
            if self
                .transfers
                .get(&id)
                .is_some_and(|t| t["state"] == "completed")
            {
                self.save(&r, track_id, host)
            } else {
                self.pending_save.insert(id, track_id);
                self.persist(host)?;
                self.start(&r, host)
            }
        } else {
            Err("Unknown action".into())
        }
    }
}
impl NativePlugin for Plugin {
    fn call(&mut self, method: &str, args: Value, host: &dyn Host) -> Result {
        let result = match method {
            "activate" => {
                self.generation = args["generation"].as_u64().unwrap_or(0);
                self.config = host.call("settings.get", json!({}))?;
                for intent in self.config["values"]["transferIntents"]
                    .as_array()
                    .into_iter()
                    .flatten()
                {
                    if intent["generation"] != self.generation || intent["result"].is_null() {
                        continue;
                    }
                    let id = key(&intent["result"]);
                    self.results.insert(id.clone(), intent["result"].clone());
                    if intent["play"] == true {
                        self.pending_play.insert(id.clone());
                    }
                    if intent["save"] == true {
                        self.pending_save.insert(id, intent["trackId"].as_i64());
                    }
                }
                self.has_password = host
                    .call("secrets.has", json!({"key":"password"}))?
                    .as_bool()
                    .unwrap_or(false);
                Ok(Value::Null)
            }
            "playback" => {
                self.playback = args;
                if self.playback["resource"]["pluginId"] == "player.soulseek" {
                    let id = text(&self.playback["resource"], "resourceId");
                    if self.pending_play.remove(&id) {
                        self.persist(host)?;
                    }
                }
                host.invalidate();
                Ok(Value::Null)
            }
            "event" => {
                let topic = text(&args, "topic");
                let payload = args["payload"].clone();
                if topic == "soulseek-cover" {
                    self.cover_workers = self.cover_workers.saturating_sub(1);
                    let id = text(&payload, "id");
                    self.cover_pending.remove(&id);
                    let hash = payload["result"].clone();
                    self.covers.insert(id.clone(), hash.clone());
                    for r in self.results.values_mut() {
                        if json!([r["username"], r["coverFilename"]]).to_string() == id {
                            r["coverHash"] = hash.clone();
                        }
                    }
                    self.covers(&[], host);
                    host.invalidate();
                    Ok(Value::Null)
                } else if topic == "soulseek-preview" || topic == "soulseek-download" {
                    self.transfer(payload, host)
                } else {
                    Ok(Value::Null)
                }
            }
            "render" => self.render(args, host),
            "renderSearch" => Ok(json!(args["ids"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|id| id.as_str().and_then(|id| self.results.get(id)))
                .map(|r| self.row(r))
                .collect::<Vec<_>>())),
            "search" => {
                let query = text(&args, "query");
                let cursor = args["cursor"].as_str();
                if cursor.is_none() {
                    let found = self.search(&query, host)?;
                    if self.searches.len() >= 4 {
                        if let Some(first) = self.searches.keys().next().cloned() {
                            self.searches.remove(&first);
                        }
                    }
                    self.searches.insert(query.clone(), found);
                }
                let offset = cursor
                    .map(|s| s.parse::<usize>().map_err(|_| "Invalid search cursor"))
                    .transpose()?
                    .unwrap_or(0);
                let limit = args["limit"].as_u64().unwrap_or(120).clamp(1, 120) as usize;
                let found = self
                    .searches
                    .get(&query)
                    .ok_or("Search expired. Run the search again.")?;
                let total = found.len();
                let page: Vec<_> = found.iter().skip(offset).take(limit).cloned().collect();
                self.covers(&page, host);
                Ok(
                    json!({"items":page.iter().map(|r|self.row(r)).collect::<Vec<_>>(),"nextCursor":if offset.saturating_add(limit)<total{Some((offset+limit).to_string())}else{None}}),
                )
            }
            "action" => {
                self.error.clear();
                self.notice.clear();
                let result = self.action(args, host);
                host.invalidate();
                result
            }
            "deactivate" => Ok(Value::Null),
            _ => Err("METHOD_NOT_FOUND".into()),
        };
        if let Err(e) = &result {
            self.error = e.clone();
            host.invalidate();
        }
        result
    }
}
#[cfg(feature = "ffi")]
player_plugin_sdk::export_plugin!(Plugin, "player.soulseek");

#[cfg(test)]
mod tests {
    use super::*;
    use player_plugin_sdk::testing::TestHost;
    fn result(i: usize) -> Value {
        json!({"username":"peer","filename":format!("folder/song-{i}.mp3"),"basename":format!("Song {i}"),"size":10_000_000,"bitrate":128,"extension":"mp3","duration":240})
    }
    fn host() -> TestHost {
        TestHost::new(|method, _| match method {
            "secrets.has" => Ok(json!(true)),
            "soulseek.status" => Ok(json!({"configured":true})),
            "soulseek.search" => Ok(json!((0..250).map(result).collect::<Vec<_>>())),
            "soulseek.prepare" => Ok(json!("source-token")),
            "library.import" | "library.replace" => Ok(json!({"id":42,"path":"song.mp3"})),
            _ => Ok(Value::Null),
        })
    }
    #[test]
    fn search_cursors_preserve_all_matches_and_replacement_opens_internal_page() {
        let h = host();
        let mut p = Plugin::default();
        p.call("activate", json!({"generation":3}), &h).unwrap();
        let mut cursor = Value::Null;
        let mut ids = BTreeSet::new();
        loop {
            let page = p
                .call(
                    "search",
                    json!({"query":"Song","cursor":cursor,"limit":120}),
                    &h,
                )
                .unwrap();
            for row in page["items"].as_array().unwrap() {
                ids.insert(text(row, "id"));
            }
            cursor = page["nextCursor"].clone();
            if cursor.is_null() {
                break;
            }
        }
        assert_eq!(ids.len(), 250);
        assert_eq!(h.count("soulseek.search"), 1);
        p.call(
            "action",
            json!({"action":"find-replacement","payload":{"track":{"id":12,"title":"Song"}}}),
            &h,
        )
        .unwrap();
        assert!(h.calls.borrow().iter().any(|(m, a)| m == "host.navigate"
            && a["pageId"] == "replacement"
            && a["context"]["track"]["id"] == 12));
        assert!(p
            .call("search", json!({"query":"Song","cursor":"invalid"}), &h)
            .is_err());
    }
    #[test]
    fn native_preview_plays_buffer_and_save_survives_instance_reload() {
        let h = host();
        let mut p = Plugin::default();
        p.call("activate", json!({"generation":7}), &h).unwrap();
        p.call("search", json!({"query":"Song"}), &h).unwrap();
        let r = result(0);
        let id = key(&r);
        p.call(
            "action",
            json!({"action":"play","payload":{"resourceId":id}}),
            &h,
        )
        .unwrap();
        let progress = json!({"username":"peer","filename":r["filename"],"state":"progress","localPath":"partial.mp3","bytesDownloaded":500_000});
        p.call(
            "event",
            json!({"topic":"soulseek-preview","payload":progress}),
            &h,
        )
        .unwrap();
        assert_eq!(h.count("host.play"), 1);
        p.call(
            "action",
            json!({"action":"save","payload":{"resourceId":id}}),
            &h,
        )
        .unwrap();
        assert_eq!(h.count("soulseek.preview"), 1);
        let mut reloaded = Plugin::default();
        reloaded
            .call("activate", json!({"generation":7}), &h)
            .unwrap();
        let completed = json!({"username":"peer","filename":r["filename"],"state":"completed","localPath":"complete.mp3","bytesDownloaded":10_000_000});
        for _ in 0..2 {
            reloaded
                .call(
                    "event",
                    json!({"topic":"soulseek-preview","payload":completed}),
                    &h,
                )
                .unwrap();
        }
        assert_eq!(h.count("library.import"), 1);
        assert_eq!(h.count("host.adopt"), 1);
        assert!(h.settings.borrow()["values"]["transferIntents"]
            .as_array()
            .unwrap()
            .is_empty());
    }
    #[test]
    fn failed_transfers_clear_intents_and_stale_generations_do_not_restore_them() {
        let h = host();
        let mut p = Plugin::default();
        p.call("activate", json!({"generation":1}), &h).unwrap();
        p.call("search", json!({"query":"Song"}), &h).unwrap();
        let r = result(0);
        let id = key(&r);
        p.call(
            "action",
            json!({"action":"save","payload":{"resourceId":id}}),
            &h,
        )
        .unwrap();
        let mut fresh = Plugin::default();
        fresh.call("activate", json!({"generation":2}), &h).unwrap();
        assert!(fresh.pending_save.is_empty());
        p.call("event",json!({"topic":"soulseek-preview","payload":{"username":"peer","filename":r["filename"],"state":"failed","error":"Peer offline"}}),&h).unwrap();
        assert!(p.pending_save.is_empty());
        assert!(h.settings.borrow()["values"]["transferIntents"]
            .as_array()
            .unwrap()
            .is_empty());
        assert_eq!(p.error, "Peer offline");
    }
}
