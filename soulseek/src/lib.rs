mod diagnostics;
mod service;
use player_plugin_sdk::{Host, NativePlugin, Result};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Default)]
pub struct Plugin {
    service: std::sync::Arc<std::sync::Mutex<service::Service>>,
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
fn cover_key(r: &Value) -> Option<String> {
    service::soulseek_parent(r["filename"].as_str()?)
        .map(|directory| json!([r["username"], directory]).to_string())
}
fn failed(t: &Value) -> bool {
    matches!(
        t["state"].as_str(),
        Some("failed" | "timed_out" | "cancelled")
    )
}
impl Plugin {
    fn log(&self, event: &'static str, id: &str) {
        if let Ok(service) = self.service.lock() { service.log(event, Some(id)); }
    }
    fn resource(&self, r: &Value) -> Value {
        let r = self.results.get(&key(r)).unwrap_or(r);
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
        let mut status = match t["state"].as_str().unwrap_or("") {
            "starting" => "Starting download",
            "queued_local" => "Waiting for connection",
            "queued_remote" => "Waiting in peer's upload queue",
            "progress" => "Downloading",
            "completed" => "Download complete",
            "cancelled" => "Cancelled",
            "timed_out" => "Download timed out",
            "failed" => "Download failed",
            state => state,
        }.to_owned();
        if let Some(bytes) = t["bytesDownloaded"].as_f64() {
            status += &format!(" · {} KiB", (bytes / 1024.).round());
        }
        if let Some(position) = t["queuePosition"].as_u64() {
            status += &format!(" · position {position}");
        }
        let playing = self.playback["playing"] == true
            && self.playback["resource"]["pluginId"] == "player.soulseek"
            && self.playback["resource"]["resourceId"] == id;
        let cover_loading = cover_key(r).is_some_and(|key| self.cover_pending.contains(&key));
        json!({"id":id,"title":r["basename"],"subtitle":subtitle,"coverHash":self.results.get(&id).map(|row|row["coverHash"].clone()).unwrap_or_else(||r["coverHash"].clone()),"coverLoading":cover_loading,"status":status,"actions":[
            {"id":"play","label":if self.pending_play.contains(&id){"Cancel preview"}else if playing{"Stop preview"}else{"Preview"}},
            {"id":"save","label":if self.busy.contains(&id){"Saving…"}else if self.pending_save.contains_key(&id){"Cancel save"}else{"Save to library"},"disabled":self.busy.contains(&id)}]})
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
        let mut result = result;
        for r in &mut result {
            if let Some(hash) = cover_key(r).and_then(|id| self.covers.get(&id)) {
                r["coverHash"] = hash.clone();
            }
            self.results.insert(key(r), r.clone());
        }
        Ok(result)
    }
    fn covers(&mut self, rows: &[Value], host: &dyn Host) {
        for r in rows {
            let Some(id) = cover_key(r) else { continue; };
            if !self.covers.contains_key(&id) && self.cover_pending.insert(id) {
                self.cover_queue.push_back(r.clone());
            }
        }
        while self.cover_workers < 2 {
            let Some(r) = self.cover_queue.pop_front() else {
                break;
            };
            let id = cover_key(&r).expect("queued cover has a parent folder");
            let task=host.call("host.background",json!({"topic":"soulseek-cover","id":id,"method":"soulseek.cover","args":{"request":{"username":r["username"],"filename":r["filename"]}}}));
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
        let result = host.call(
            "host.play",
            json!({"sourceToken":token,"resource":self.resource(r)}),
        );
        self.log(if result.is_ok() { "preview_started" } else { "preview_failed" }, &key(r));
        result
    }
    fn save(&mut self, r: &Value, track_id: Option<i64>, host: &dyn Host) -> Result {
        let id = key(r);
        if !self.busy.insert(id.clone()) {
            return Ok(Value::Null);
        }
        self.log("import_started", &id);
        host.invalidate();
        let saved = (|| {
            let token = self.prepare(r, host)?;
            let track = host.call(
                if track_id.is_some() {
                    "library.replace"
                } else {
                    "library.import"
                },
                json!({"sourceToken":token,"trackId":track_id,"coverHash":self.resource(r)["coverHash"]}),
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
        self.log(if saved.is_ok() { "import_completed" } else { "import_failed" }, &id);
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
        match host.call("soulseek.preview",json!({"request":{"username":r["username"],"filename":r["filename"],"size":r["size"]}})) {
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
            self.log("preview_ready", &id);
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
            let mut items: Vec<_> = self.transfers.iter().filter_map(|(id, t)| {
                matches!(t["state"].as_str(), Some("starting" | "queued_local" | "queued_remote" | "progress"))
                    .then(|| self.results.get(id).map(|r| self.row(r))).flatten()
            }).collect();
            let count = items.len();
            let completed_preview = (self.playback["resource"]["pluginId"] == "player.soulseek")
                .then(|| text(&self.playback["resource"], "resourceId"))
                .filter(|id| self.transfers.get(id).is_some_and(|t| t["state"] == "completed"));
            if let Some(r) = completed_preview.as_ref().and_then(|id| self.results.get(id)) {
                items.push(self.row(r));
            }
            let (tone, label) = if count > 0 {
                ("active", format!("Soulseek · {count} active transfer{}", if count == 1 { "" } else { "s" }))
            } else if self.transfers.values().any(|t| matches!(t["state"].as_str(), Some("failed" | "timed_out"))) {
                ("error", "Soulseek · Download failed".into())
            } else if completed_preview.is_some() {
                ("success", "Soulseek · Download complete".into())
            } else {
                return Ok(Value::Null);
            };
            return Ok(json!({"type":"status","tone":tone,"label":label,"title":"Soulseek","children":[
                {"type":"results","items":items}
            ]}));
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
                if self.pending_save.contains_key(&id) {
                    return self.persist(host);
                }
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
            if self.pending_save.remove(&id).is_some() {
                self.persist(host)?;
                if !self.pending_play.contains(&id) && !self.busy.contains(&id) {
                    self.transfers.insert(id.clone(), json!({"state":"cancelled"}));
                    self.tokens.remove(&id);
                    return host.call("soulseek.cancelPreview", json!({"request":{"username":r["username"],"filename":r["filename"]}}));
                }
                return Ok(Value::Null);
            }
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
impl Plugin {
    fn handle(&mut self, method: &str, args: Value, host: &dyn Host) -> Result {
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
                    if payload["retryable"] != true { self.covers.insert(id.clone(), hash.clone()); }
                    for r in self.results.values_mut() {
                        if cover_key(r).as_deref() == Some(&id) {
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
impl NativePlugin for Plugin {
    fn call(&mut self, method: &str, args: Value, host: &dyn Host) -> Result {
        let services = service::Services {
            service: self.service.clone(),
            host,
        };
        if method == "activate" {
            self.service
                .lock()
                .map_err(|e| e.to_string())?
                .activate(host)?;
        }
        if method == "deactivate" {
            self.service.lock().map_err(|e| e.to_string())?.close();
            return Ok(Value::Null);
        }
        if method == "poll" {
            let events = self.service.lock().map_err(|e| e.to_string())?.drain();
            for mut event in events {
                if event["topic"] == "soulseek-cover-discovered" {
                    let started = self.service.lock().map_err(|e| e.to_string())?
                        .discovered_cover(&event["payload"], host);
                    if started.is_err() {
                        self.handle("event", json!({"topic":"soulseek-cover","payload":{"id":event["payload"]["id"],"result":null,"retryable":true}}), &services)?;
                    }
                    continue;
                }
                if event["topic"] == "soulseek-cover" {
                    if let Some(path) = event["payload"]["path"].as_str() {
                        event["payload"]["result"] = host
                            .call("media.cover", json!({"path":path}))
                            .unwrap_or(Value::Null);
                        let outcome = if event["payload"]["result"].is_null() {
                            "cover_media_unavailable"
                        } else {
                            "cover_media_ready"
                        };
                        self.service.lock().map_err(|e| e.to_string())?.log(
                            outcome, event["payload"]["id"].as_str(),
                        );
                    }
                } else if let Some(path) = event["payload"]["localPath"].as_str() {
                    let state = event["payload"]["state"].as_str().unwrap_or("active");
                    let state =
                        if ["completed", "failed", "cancelled", "timed_out"].contains(&state) {
                            state
                        } else {
                            "active"
                        };
                    host.call("media.state", json!({"path":path,"state":state}))?;
                }
                self.handle("event", event, &services)?;
            }
            return Ok(Value::Null);
        }
        self.handle(method, args, &services)
    }
}

#[cfg(feature = "ffi")]
player_plugin_sdk::export_plugin!(Plugin, "player.soulseek");

#[cfg(test)]
mod tests {
    use super::*;
    use player_plugin_sdk::testing::TestHost;
    #[test]
    fn status_menu_exposes_transfer_actions() {
        let h = host();
        let mut p = Plugin::default();
        let r = result(0);
        let id = key(&r);
        p.results.insert(id.clone(), r.clone());
        p.action(json!({"action":"play","payload":{"resourceId":id}}), &h).unwrap();
        let menu = p.render(json!({"viewId":"buffer"}), &h).unwrap();
        let row = &menu["children"][0]["items"][0];
        assert_eq!(menu["title"], "Soulseek");
        assert_eq!(row["title"], r["basename"]);
        assert_eq!(row["status"], "Starting download · 0 KiB");
        assert_eq!(row["actions"][0]["label"], "Cancel preview");
        assert_eq!(row["actions"][1]["label"], "Save to library");
        p.action(json!({"action":row["actions"][1]["id"],"payload":{"resourceId":row["id"]}}), &h).unwrap();
        let menu = p.render(json!({"viewId":"buffer"}), &h).unwrap();
        let row = &menu["children"][0]["items"][0];
        assert_eq!(row["actions"][1]["label"], "Cancel save");
        p.action(json!({"action":row["actions"][1]["id"],"payload":{"resourceId":row["id"]}}), &h).unwrap();
        assert!(p.pending_play.contains(&id));
        assert_eq!(h.count("soulseek.cancelPreview"), 0);
        p.action(json!({"action":row["actions"][0]["id"],"payload":{"resourceId":row["id"]}}), &h).unwrap();
        assert_eq!(h.count("soulseek.cancelPreview"), 1);
        assert!(p.render(json!({"viewId":"buffer"}), &h).unwrap().is_null());
    }
    #[test]
    fn completed_preview_status_follows_active_track_without_hiding_downloads() {
        let h = host();
        let mut p = Plugin::default();
        let r = result(0);
        let id = key(&r);
        p.results.insert(id.clone(), r.clone());
        p.transfers.insert(id.clone(), json!({"state":"completed","bytesDownloaded":10_000_000}));
        p.handle("playback", json!({"playing":true,"resource":{"pluginId":"player.soulseek","resourceId":id}}), &h).unwrap();
        let menu = p.render(json!({"viewId":"buffer"}), &h).unwrap();
        assert_eq!(menu["tone"], "success");
        assert_eq!(menu["label"], "Soulseek · Download complete");
        assert_eq!(menu["children"][0]["items"], json!([p.row(&r)]));
        assert_eq!(menu["children"][0]["items"][0]["status"], "Download complete · 9766 KiB");
        assert_eq!(menu["children"][0]["items"][0]["actions"], json!([
            {"id":"play","label":"Stop preview"},
            {"id":"save","label":"Save to library","disabled":false}
        ]));
        p.handle("playback", json!({"playing":true,"resource":{"pluginId":"other.plugin","resourceId":id}}), &h).unwrap();
        assert!(p.render(json!({"viewId":"buffer"}), &h).unwrap().is_null());
        p.transfers.insert(id, json!({"state":"progress"}));
        let menu = p.render(json!({"viewId":"buffer"}), &h).unwrap();
        assert_eq!(menu["tone"], "active");
        assert_eq!(menu["label"], "Soulseek · 1 active transfer");
        assert_eq!(menu["children"][0]["items"], json!([p.row(&r)]));
        assert_eq!(h.count("soulseek.preview"), 0);
    }

    #[test]
    fn missing_search_images_discover_once_per_peer_folder_and_refresh_all_rows() {
        let h = TestHost::new(|_, _| Ok(Value::Null));
        let a = result(0);
        let b = result(1);
        let mut other = result(2);
        other["username"] = json!("other");
        let mut next = result(3);
        next["filename"] = json!("next/song.mp3");
        let mut p = Plugin::default();
        for r in [&a, &b, &other, &next] { p.results.insert(key(r), r.clone()); }
        p.covers(&[a.clone(), b.clone(), other.clone(), next.clone()], &h);
        assert_eq!(h.count("host.background"), 2);
        assert_eq!(p.cover_queue.len(), 1);
        assert_eq!(p.cover_workers, 2);
        assert_eq!(p.row(&a)["coverLoading"], true);
        assert_eq!(p.row(&b)["coverLoading"], true);
        assert!(h.calls.borrow().iter().any(|(method, args)| method == "host.background" && args["args"]["request"]["filename"] == a["filename"]));
        p.handle("event", json!({"topic":"soulseek-cover","payload":{"id":cover_key(&a).unwrap(),"result":"real-hash"}}), &h).unwrap();
        assert_eq!(p.row(&a)["coverHash"], "real-hash");
        assert_eq!(p.row(&b)["coverHash"], "real-hash");
        assert_eq!(p.row(&a)["coverLoading"], false);
        assert_eq!(p.row(&b)["coverLoading"], false);
        assert!(p.row(&other)["coverHash"].is_null());
        assert_eq!(h.count("host.background"), 3);
        p.covers(&[a, b], &h);
        assert_eq!(h.count("host.background"), 3);
    }
    #[test]
    fn transient_folder_error_retries_on_next_search_but_empty_folder_is_cached() {
        let h = TestHost::new(|_, _| Ok(Value::Null));
        let row = result(0);
        let id = cover_key(&row).unwrap();
        let mut p = Plugin::default();
        p.covers(&[row.clone()], &h);
        p.handle("event", json!({"topic":"soulseek-cover","payload":{"id":id,"result":null,"retryable":true}}), &h).unwrap();
        assert_eq!(p.row(&row)["coverLoading"], false);
        p.covers(&[row.clone()], &h);
        assert_eq!(h.count("host.background"), 2);
        p.handle("event", json!({"topic":"soulseek-cover","payload":{"id":id,"result":null}}), &h).unwrap();
        assert_eq!(p.row(&row)["coverLoading"], false);
        p.covers(&[row], &h);
        assert_eq!(h.count("host.background"), 2);
    }
    #[test]
    fn cached_covers_survive_search_refresh_and_reach_imports() {
        let r = result(0);
        let found = r.clone();
        let h = TestHost::new(move |method, _| match method {
            "soulseek.status" => Ok(json!({"configured":true})),
            "soulseek.search" => Ok(json!([found.clone()])),
            "soulseek.prepare" => Ok(json!("source-token")),
            "library.import" => Ok(json!({"id":42})),
            _ => Ok(Value::Null),
        });
        let mut p = Plugin::default();
        let cover_id = cover_key(&r).unwrap();
        p.search("Song", &h).unwrap();
        p.handle("event", json!({"topic":"soulseek-cover","payload":{"id":cover_id,"result":"folder-hash"}}), &h).unwrap();
        let refreshed = p.search("Song", &h).unwrap();
        assert_eq!(refreshed[0]["coverHash"], "folder-hash");
        assert_eq!(p.row(&r)["coverHash"], "folder-hash");
        assert_eq!(p.resource(&r)["coverHash"], "folder-hash");
        p.save(&r, None, &h).unwrap();
        assert!(h.calls.borrow().iter().any(|(method, args)| method == "library.import" && args["coverHash"] == "folder-hash"));
    }

    #[test]
    fn remote_queue_cancel_preview_preserves_save() {
        let h = host();
        let mut p = Plugin::default();
        let r = result(0);
        let id = key(&r);
        p.results.insert(id.clone(), r.clone());
        for action in ["play", "save"] {
            p.action(json!({"action":action,"payload":{"resourceId":id}}), &h).unwrap();
        }
        p.transfer(json!({"username":"peer","filename":r["filename"],"state":"queued_remote","queuePosition":3,"localPath":"partial.mp3"}), &h).unwrap();
        assert_eq!(h.count("host.play"), 0);
        assert_eq!(p.row(&r)["status"], "Waiting in peer's upload queue · position 3");
        assert_eq!(p.row(&r)["actions"][1]["label"], "Cancel save");
        assert_eq!(p.render(json!({"viewId":"buffer"}), &h).unwrap()["tone"], "active");
        p.action(json!({"action":"play","payload":{"resourceId":id}}), &h).unwrap();
        assert_eq!(h.count("soulseek.cancelPreview"), 0);
        assert!(p.pending_save.contains_key(&id));
        p.transfer(json!({"username":"peer","filename":r["filename"],"state":"completed","localPath":"complete.mp3"}), &h).unwrap();
        assert_eq!(h.count("library.import"), 1);
        assert_eq!(h.count("host.play"), 0);
        assert!(p.render(json!({"viewId":"buffer"}), &h).unwrap().is_null());
    }
    #[test]
    fn cancelling_only_save_cancels_owned_transfer() {
        let h = host();
        let mut p = Plugin::default();
        let r = result(0);
        let id = key(&r);
        p.results.insert(id.clone(), r);
        for _ in 0..2 {
            p.action(json!({"action":"save","payload":{"resourceId":id}}), &h).unwrap();
        }
        assert!(p.pending_save.is_empty());
        assert_eq!(h.count("soulseek.preview"), 1);
        assert_eq!(h.count("soulseek.cancelPreview"), 1);
        assert!(p.render(json!({"viewId":"buffer"}), &h).unwrap().is_null());
    }
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
        p.handle("activate", json!({"generation":3}), &h).unwrap();
        let mut cursor = Value::Null;
        let mut ids = BTreeSet::new();
        loop {
            let page = p
                .handle(
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
        p.handle(
            "action",
            json!({"action":"find-replacement","payload":{"track":{"id":12,"title":"Song"}}}),
            &h,
        )
        .unwrap();
        assert!(h.calls.borrow().iter().any(|(m, a)| m == "host.navigate"
            && a["pageId"] == "replacement"
            && a["context"]["track"]["id"] == 12));
        assert!(p
            .handle("search", json!({"query":"Song","cursor":"invalid"}), &h)
            .is_err());
    }
    #[test]
    fn native_preview_plays_buffer_and_save_survives_instance_reload() {
        let h = host();
        let mut p = Plugin::default();
        p.handle("activate", json!({"generation":7}), &h).unwrap();
        p.handle("search", json!({"query":"Song"}), &h).unwrap();
        let r = result(0);
        let id = key(&r);
        p.handle(
            "action",
            json!({"action":"play","payload":{"resourceId":id}}),
            &h,
        )
        .unwrap();
        let progress = json!({"username":"peer","filename":r["filename"],"state":"progress","localPath":"partial.mp3","bytesDownloaded":500_000});
        p.handle(
            "event",
            json!({"topic":"soulseek-preview","payload":progress}),
            &h,
        )
        .unwrap();
        assert_eq!(h.count("host.play"), 1);
        p.handle(
            "action",
            json!({"action":"save","payload":{"resourceId":id}}),
            &h,
        )
        .unwrap();
        assert_eq!(h.count("soulseek.preview"), 1);
        let mut reloaded = Plugin::default();
        reloaded
            .handle("activate", json!({"generation":7}), &h)
            .unwrap();
        let completed = json!({"username":"peer","filename":r["filename"],"state":"completed","localPath":"complete.mp3","bytesDownloaded":10_000_000});
        for _ in 0..2 {
            reloaded
                .handle(
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
        p.handle("activate", json!({"generation":1}), &h).unwrap();
        p.handle("search", json!({"query":"Song"}), &h).unwrap();
        let r = result(0);
        let id = key(&r);
        p.handle(
            "action",
            json!({"action":"save","payload":{"resourceId":id}}),
            &h,
        )
        .unwrap();
        let mut fresh = Plugin::default();
        fresh
            .handle("activate", json!({"generation":2}), &h)
            .unwrap();
        assert!(fresh.pending_save.is_empty());
        p.handle("event",json!({"topic":"soulseek-preview","payload":{"username":"peer","filename":r["filename"],"state":"failed","error":"Peer offline"}}),&h).unwrap();
        assert!(p.pending_save.is_empty());
        assert!(h.settings.borrow()["values"]["transferIntents"]
            .as_array()
            .unwrap()
            .is_empty());
        assert_eq!(p.error, "Peer offline");
    }
}
