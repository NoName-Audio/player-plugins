//! Package-owned Soulseek protocol client. No Player/Tauri dependency.
use player_plugin_sdk::{Host, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use soulseek_rs::{Client, DownloadCanceller, DownloadStatus, File};
use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "ogg", "opus", "aac", "m4a", "wav", "wv", "ape", "alac", "aiff",
];
const COVER_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp"];
#[derive(Default)]
pub struct Service {
    runtime: Option<tokio::runtime::Runtime>,
    root: PathBuf,
    client: Option<Client>,
    config: Option<(String, String)>,
    previews: HashMap<String, (PathBuf, Arc<Mutex<String>>)>,
    jobs: HashMap<String, DownloadCanceller>,
    events: Arc<Mutex<BTreeMap<String, Value>>>,
    pub diagnostics: Arc<Mutex<crate::diagnostics::Diagnostics>>,
}
fn resource_key(user: &str, file: &str) -> String {
    json!([user, file]).to_string()
}
fn basename(file: &str) -> &str {
    file.rsplit(['\\', '/']).next().unwrap_or(file)
}
fn validate_file(file: &str) -> Result<()> {
    let name = basename(file);
    if file.is_empty()
        || file.len() > 8192
        || file.contains('\0')
        || name.is_empty()
        || [".", ".."].contains(&name)
        || name
            .chars()
            .any(|c| c.is_control() || "<>:\"|?*".contains(c))
    {
        return Err("Invalid remote filename".into());
    }
    Ok(())
}
impl Service {
    pub fn log(&self, event: &'static str, resource: Option<&str>) {
        self.diagnostics.lock().unwrap().record(event, resource, None, None, None);
    }
    fn runtime(&mut self) -> Result<&tokio::runtime::Runtime> {
        if self.runtime.is_none() {
            self.runtime = Some(
                tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .enable_all()
                    .build()
                    .map_err(|e| e.to_string())?,
            );
        }
        Ok(self.runtime.as_ref().unwrap())
    }
    pub fn activate(&mut self, host: &dyn Host) -> Result<()> {
        let root = host.call("storage.staging", json!({}))?;
        self.root = PathBuf::from(root["path"].as_str().ok_or("Invalid staging directory")?);
        *self.diagnostics.lock().unwrap() = crate::diagnostics::Diagnostics::open(&self.root).map_err(|e| e.to_string())?;
        self.log("activated", None);
        Ok(())
    }
    fn client(&mut self, host: &dyn Host) -> Result<Client> {
        let settings = host.call("settings.get", json!({}))?;
        let username = settings["values"]["username"]
            .as_str()
            .unwrap_or("")
            .trim()
            .to_owned();
        if username.is_empty() {
            return Err("Add your Soulseek username in plugin settings first".into());
        }
        let password = host
            .call("secrets.get", json!({"key":"password"}))?
            .as_str()
            .unwrap_or("")
            .to_owned();
        if password.is_empty() {
            return Err("Save your Soulseek password in plugin settings first".into());
        }
        let config = (username, password);
        if self.config.as_ref() != Some(&config) {
            self.cancel_all();
            self.client = None;
            self.config = Some(config.clone());
        }
        if self.client.is_none() {
            let guard = self.runtime()?.enter();
            let client = Client::new(config.0, config.1);
            drop(guard);
            self.client = Some(client);
        }
        let client = self.client.as_ref().unwrap().clone();
        self.log("connection_attempt", None);
        let connected = self.runtime()?.block_on(client.connect());
        self.log(if connected.is_ok() { "connected" } else { "connection_failed" }, None);
        connected.map_err(|e| e.to_string())?;
        Ok(client)
    }
    fn cancel_all(&mut self) {
        for (_, job) in self.jobs.drain() {
            job.cancel();
        }
    }
    pub fn close(&mut self) {
        self.cancel_all();
        self.log("closed", None);
        self.client = None;
        self.config = None;
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
    pub fn drain(&mut self) -> Vec<Value> {
        std::mem::take(&mut *self.events.lock().unwrap())
            .into_values()
            .collect()
    }
    pub fn call(&mut self, method: &str, args: Value, host: &dyn Host) -> Result {
        match method {
            "soulseek.status" => {
                let settings = host.call("settings.get", json!({}))?;
                let user = settings["values"]["username"].as_str().unwrap_or("").trim();
                let saved = host.call("secrets.has", json!({"key":"password"}))? == true;
                Ok(
                    json!({"enabled":true,"configured":!user.is_empty()&&saved,"username":user,"activeSession":self.client.is_some()}),
                )
            }
            "soulseek.search" => {
                let query = args["query"].as_str().unwrap_or("").trim();
                if query.is_empty() {
                    return Ok(json!([]));
                }
                let client = self.client(host)?;
                let results = self
                    .runtime()?
                    .block_on(client.search(query, Duration::from_secs(10)))
                    .map_err(|e| e.to_string())?;
                serde_json::to_value(flatten_search_results(results)).map_err(|e| e.to_string())
            }
            "soulseek.preview" => {
                let request: SoulseekDownloadRequest =
                    serde_json::from_value(args["request"].clone()).map_err(|e| e.to_string())?;
                self.start(request, None, host)
            }
            "soulseek.cancelPreview" => {
                let user = args["request"]["username"].as_str().unwrap_or("");
                let file = args["request"]["filename"].as_str().unwrap_or("");
                let id = resource_key(user, file);
                self.log("cancel_requested", Some(&id));
                Ok(json!(self
                    .jobs
                    .remove(&id)
                    .map(|job| job.cancel())
                    .is_some()))
            }
            "soulseek.prepare" => {
                let user = args["username"].as_str().ok_or("Missing peer username")?;
                let file = args["filename"].as_str().ok_or("Missing remote file")?;
                let (path, state) = self
                    .previews
                    .get(&resource_key(user, file))
                    .ok_or("Preview has not been downloaded")?;
                host.call("media.register",json!({"path":path,"resource":args["resource"],"state":state.lock().unwrap().clone()}))
            }
            _ => Err("Unknown package-owned Soulseek service".into()),
        }
    }
    pub fn cover(&mut self, args: Value, host: &dyn Host) -> Result {
        let request = &args["args"]["request"];
        let user = request["username"].as_str().ok_or("Missing peer username")?.to_owned();
        let filename = request["filename"].as_str().ok_or("Missing track filename")?;
        validate_file(filename)?;
        let directory = soulseek_parent(filename).ok_or("Track has no shared parent folder")?;
        let client = self.client(host)?;
        let events = self.events.clone();
        let diagnostics = self.diagnostics.clone();
        let id = args["id"].clone();
        self.runtime()?.spawn(async move {
            let files = client.folder_contents(&user, &directory, Duration::from_secs(20)).await;
            let retryable = files.is_err();
            let selected = match files {
                Ok(files) => {
                    diagnostics.lock().unwrap().record("artwork_folder_listed", id.as_str(), None, None, None);
                    select_folder_cover(files, &user, &directory)
                }
                Err(error) => {
                    let category = match &error {
                        soulseek_rs::SoulseekRs::Timeout => "timeout",
                        soulseek_rs::SoulseekRs::NotConnected => "not_connected",
                        soulseek_rs::SoulseekRs::ConnectionClosed => "connection_closed",
                        soulseek_rs::SoulseekRs::InvalidMessage(phase) if phase == "folder_timeout_worker_pending" => "folder_timeout_worker_pending",
                        soulseek_rs::SoulseekRs::InvalidMessage(phase) if phase == "folder_timeout_rendezvous_requested" => "folder_timeout_rendezvous_requested",
                        soulseek_rs::SoulseekRs::InvalidMessage(phase) if phase == "folder_timeout_peer_connected" => "folder_timeout_peer_connected",
                        soulseek_rs::SoulseekRs::InvalidMessage(phase) if phase == "folder_timeout_request_sent" => "folder_timeout_request_sent",
                        soulseek_rs::SoulseekRs::InvalidMessage(phase) if phase == "folder_timeout_indirect_requested" => "folder_timeout_indirect_requested",
                        soulseek_rs::SoulseekRs::ParseError(_) | soulseek_rs::SoulseekRs::InvalidMessage(_) | soulseek_rs::SoulseekRs::CompressionError(_) => "invalid_folder_response",
                        _ => "folder_network_error",
                    };
                    diagnostics.lock().unwrap().record_with_category("artwork_folder_failed", id.as_str(), None, None, None, Some(category));
                    None
                }
            };
            diagnostics.lock().unwrap().record(
                if selected.is_some() { "artwork_folder_found" } else { "artwork_folder_unavailable" },
                id.as_str(), None, None, None,
            );
            let event = match selected {
                Some(file) => json!({"topic":"soulseek-cover-discovered","payload":{"id":id,"username":user,"filename":file.filename,"size":file.size}}),
                None => json!({"topic":"soulseek-cover","payload":{"id":id,"result":null,"retryable":retryable}}),
            };
            events.lock().unwrap().insert(format!("folder:{id}"), event);
        });
        Ok(Value::Null)
    }
    pub fn discovered_cover(&mut self, payload: &Value, host: &dyn Host) -> Result {
        let request = SoulseekDownloadRequest {
            username: payload["username"].as_str().ok_or("Missing cover peer")?.into(),
            filename: payload["filename"].as_str().ok_or("Missing cover path")?.into(),
            size: payload["size"].as_u64().ok_or("Missing cover size")?,
        };
        self.start(request, Some(payload["id"].clone()), host)
    }
    fn start(
        &mut self,
        request: SoulseekDownloadRequest,
        cover: Option<Value>,
        host: &dyn Host,
    ) -> Result {
        validate_file(&request.filename)?;
        if self.root.as_os_str().is_empty() {
            return Err("Plugin staging directory is not initialized".into());
        }
        let client = self.client(host)?;
        let id = resource_key(&request.username, &request.filename);
        self.log(if cover.is_some() { "artwork_enqueue" } else { "enqueue" }, Some(&id));
        let directory = if cover.is_some() {
            self.root
                .join("covers")
                .join(format!("{:x}", Sha256::digest(id.as_bytes())))
        } else {
            self.root.join(format!(
                "preview-{}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            ))
        };
        std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        let path = directory.join(basename(&request.filename));
        if let Some(cover_id) = &cover {
            if path
                .metadata()
                .is_ok_and(|m| m.is_file() && m.len() == request.size)
            {
                self.log("artwork_cached", Some(&id));
                self.events.lock().unwrap().insert(
                    format!("cover:{id}"),
                    json!({"topic":"soulseek-cover","payload":{"id":cover_id,"path":path}}),
                );
                return Ok(Value::Null);
            }
        }
        // A stable owned path exists before registration, even when queued remotely.
        std::fs::File::create(&path).map_err(|e| e.to_string())?;
        let guard = self.runtime()?.enter();
        let (_download, mut handle) = client
            .download(
                soulseek_rs::SoulseekPath::from_wire(request.filename.clone()),
                request.username.clone(),
                request.size,
                directory.to_string_lossy().into(),
                Some(Duration::from_secs(30)),
                Some(Duration::from_secs(600)),
            )
            .map_err(|e| e.to_string())?;
        drop(guard);
        self.jobs.insert(id.clone(), handle.cancel_handle());
        let state = Arc::new(Mutex::new("active".to_owned()));
        if cover.is_none() {
            self.previews
                .insert(id.clone(), (path.clone(), state.clone()));
        }
        let events = self.events.clone();
        let diagnostics = self.diagnostics.clone();
        self.runtime()?.spawn(async move {
   let mut terminal=false;
   let mut last_log = std::time::Instant::now() - Duration::from_secs(5);
   let mut last_state = "";
   let mut last_queue = Value::Null;
   while let Some(status)=handle.recv().await{
    let mut event=json!({"username":request.username,"filename":request.filename,"basename":basename(&request.filename),"localPath":path,"totalBytes":request.size});
    let mut failure_category = None;
    if let DownloadStatus::Protocol { phase, bytes, total } = &status {
     diagnostics.lock().unwrap().record(phase,Some(&id),Some(*bytes),Some(*total),None);
     continue;
    }
    let state_name=match status {
     DownloadStatus::Protocol { .. } => unreachable!("handled above"),
     DownloadStatus::QueuedLocally=>"queued_local",
     DownloadStatus::QueuedRemotely{place}=>{event["queuePosition"]=json!(place);"queued_remote"},
     DownloadStatus::InProgress{bytes_downloaded,total_bytes,speed_bytes_per_sec}=>{event["bytesDownloaded"]=json!(bytes_downloaded);event["totalBytes"]=json!(total_bytes);event["speedBytesPerSec"]=json!(speed_bytes_per_sec);"progress"},
     DownloadStatus::Completed=>{terminal=true;"completed"},DownloadStatus::Failed{category}=>{terminal=true;failure_category=Some(category);event["error"]=json!("Download failed");"failed"},
     DownloadStatus::TimedOut=>{terminal=true;failure_category=Some("no_progress_timeout");event["error"]=json!("Download timed out");"timed_out"},DownloadStatus::Cancelled=>{terminal=true;"cancelled"},
    };
    if let Some(category) = failure_category { event["failureCategory"] = json!(category); }
    event["state"]=json!(state_name);*state.lock().unwrap()=if terminal{state_name}else{"active"}.into();
    if state_name != last_state || event["queuePosition"] != last_queue || last_log.elapsed() >= Duration::from_secs(5) {
     diagnostics.lock().unwrap().record_with_category(if cover.is_some() { match state_name { "completed" => "artwork_completed", "failed" => "artwork_failed", "timed_out" => "artwork_timed_out", "cancelled" => "artwork_cancelled", _ => "artwork_transfer" } } else { state_name },Some(&id),event["bytesDownloaded"].as_u64(),event["totalBytes"].as_u64(),event["queuePosition"].as_u64(),failure_category);
     last_log=std::time::Instant::now();last_state=state_name;last_queue=event["queuePosition"].clone();
    }
    if let Some(cover_id)=&cover {
     if terminal{events.lock().unwrap().insert(format!("cover:{id}"),json!({"topic":"soulseek-cover","payload":{"id":cover_id,"path":if state_name=="completed"{Some(path.clone())}else{None},"result":null,"failureCategory":failure_category}}));}
    }else{events.lock().unwrap().insert(format!("transfer:{id}"),json!({"topic":"soulseek-preview","payload":event}));}
    if terminal{break;}
   }
   if !terminal { diagnostics.lock().unwrap().record_with_category("transfer_stream_ended",Some(&id),None,None,None,Some("transfer_stream_ended")); }
   if !terminal{*state.lock().unwrap()="failed".into();let value=if let Some(cover_id)=cover{json!({"topic":"soulseek-cover","payload":{"id":cover_id,"result":null,"failureCategory":"transfer_stream_ended"}})}else{json!({"topic":"soulseek-preview","payload":{"username":request.username,"filename":request.filename,"localPath":path,"state":"failed","error":"Transfer ended unexpectedly","failureCategory":"transfer_stream_ended"}})};events.lock().unwrap().insert(format!("ended:{id}"),value);}
  });
        Ok(Value::Null)
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        self.close();
    }
}
pub struct Services<'a> {
    pub service: Arc<Mutex<Service>>,
    pub host: &'a dyn Host,
}
impl Host for Services<'_> {
    fn call(&self, method: &str, args: Value) -> Result {
        if method.starts_with("soulseek.") {
            return self
                .service
                .lock()
                .map_err(|e| e.to_string())?
                .call(method, args, self.host);
        }
        if method == "host.background" && args["method"] == "soulseek.cover" {
            return self
                .service
                .lock()
                .map_err(|e| e.to_string())?
                .cover(args, self.host);
        }
        self.host.call(method, args)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SoulseekDownloadRequest {
    username: String,
    filename: String,
    size: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SoulseekSearchResult {
    pub username: String,
    pub filename: String,
    pub basename: String,
    pub size: u64,
    pub bitrate: Option<u32>,
    pub duration: Option<u32>,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<u32>,
    pub vbr: Option<bool>,
    pub peer_speed: u32,
    pub free_upload_slots: u8,
    pub extension: Option<String>,
}


struct PendingCoverFile {
    filename: String,
    size: u64,
}

fn select_folder_cover(files: Vec<File>, user: &str, directory: &str) -> Option<PendingCoverFile> {
    files.into_iter().filter_map(|file| {
        let filename = file.name.to_string();
        if file.username != user || soulseek_parent(&filename).as_deref() != Some(directory)
            || file.size == 0 || file.size > 32 * 1024 * 1024 || validate_file(&filename).is_err() {
            return None;
        }
        let name = basename(&filename);
        let extension = Path::new(name).extension()?.to_str()?.to_ascii_lowercase();
        if !COVER_EXTENSIONS.contains(&extension.as_str()) { return None; }
        Some(PendingCoverFile { filename, size: file.size })
    }).next()
}

fn flatten_search_results(results: Vec<soulseek_rs::SearchResult>) -> Vec<SoulseekSearchResult> {
    let mut files = Vec::new();
    for result in results {
        for file in result.files {
            let mapped = map_search_file(file, result.slots, result.speed);
            if is_audio_result(&mapped) { files.push(mapped); }
        }
    }
    files.sort_by(|left, right| {
        right
            .free_upload_slots
            .cmp(&left.free_upload_slots)
            .then_with(|| right.peer_speed.cmp(&left.peer_speed))
            .then_with(|| left.basename.cmp(&right.basename))
            .then_with(|| left.username.cmp(&right.username))
    });

    files
}

fn map_search_file(file: File, free_upload_slots: u8, peer_speed: u32) -> SoulseekSearchResult {
    let basename = file.name.filename().to_string();
    let filename = file.name.to_string();
    let extension = Path::new(&basename)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());

    SoulseekSearchResult {
            username: file.username,
            filename,
            basename,
            size: file.size,
            bitrate: file.attributes.bitrate,
            duration: file.attributes.duration,
            sample_rate: file.attributes.sample_rate,
            bit_depth: file.attributes.bit_depth,
            vbr: file.attributes.vbr,
            peer_speed,
            free_upload_slots,
            extension,
    }
}

fn is_audio_result(result: &SoulseekSearchResult) -> bool {
    result
        .extension
        .as_deref()
        .map(|extension| AUDIO_EXTENSIONS.contains(&extension))
        .unwrap_or(false)
        || result.bitrate.is_some()
        || result.duration.is_some()
        || result.sample_rate.is_some()
}


pub(crate) fn soulseek_parent(filename: &str) -> Option<String> {
    let mut segments: Vec<&str> = filename
        .split(['\\', '/'])
        .filter(|segment| !segment.is_empty())
        .collect();
    if segments.len() <= 1 {
        return None;
    }
    segments.pop();
    Some(segments.join("\\"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use player_plugin_sdk::testing::TestHost;
    #[test]
    fn track_search_does_not_need_to_advertise_folder_image() {
        let found = flatten_search_results(vec![soulseek_rs::SearchResult {
            token: soulseek_rs::token::SearchToken(1), username: "peer".into(), slots: 1, speed: 100,
            files: vec![shared_file("peer", "Music\\Album\\song.mp3", 1000)],
        }]);
        assert_eq!(found.len(), 1);
        assert_eq!(soulseek_parent(&found[0].filename).as_deref(), Some("Music\\Album"));
        let image = select_folder_cover(vec![shared_file("peer", "Music\\Album\\COVER.PNG", 123)], "peer", "Music\\Album").unwrap();
        assert_eq!(image.filename, "Music\\Album\\COVER.PNG");
    }
    fn shared_file(user: &str, name: &str, size: u64) -> File {
        File { username: user.into(), name: soulseek_rs::SoulseekPath::from_wire(name.into()), size, attributes: Default::default() }
    }
    #[test]
    fn folder_artwork_selection_is_peer_and_directory_scoped() {
        let files = vec![
            shared_file("other", "Music\\Album\\cover.jpg", 100),
            shared_file("peer", "Music\\Album2\\cover.jpg", 100),
            shared_file("peer", "Music\\Album\\disc\\cover.jpg", 100),
            shared_file("peer", "Music\\Album\\cover.jpg", 0),
            shared_file("peer", "Music\\Album\\cover.png", 33 * 1024 * 1024),
            shared_file("peer", "Music\\Album\\song.mp3", 100),
            shared_file("peer", "Music\\Album\\Artwork.webp", 100),
            shared_file("peer", "Music\\Album\\FRONT.PNG", 100),
            shared_file("peer", "Music\\Album\\Folder.JPEG", 100),
        ];
        let chosen = select_folder_cover(files, "peer", "Music\\Album").unwrap();
        assert_eq!(chosen.filename, "Music\\Album\\Artwork.webp");
        assert_eq!(chosen.size, 100);
        let fallback = select_folder_cover(vec![shared_file("peer", "Music\\Album\\scan.WeBp", 123)], "peer", "Music\\Album").unwrap();
        assert_eq!(fallback.filename, "Music\\Album\\scan.WeBp");
        assert!(select_folder_cover(vec![shared_file("peer", "Music\\Album2\\cover.jpg", 100)], "peer", "Music\\Album").is_none());
    }
    #[test]
    fn status_checks_presence_without_reading_password_or_connecting() {
        let h = TestHost::new(|method, _| {
            if method == "secrets.has" {
                Ok(json!(true))
            } else {
                Err("Unexpected host call".into())
            }
        });
        *h.settings.borrow_mut() = json!({"revision":0,"values":{"username":"listener"}});
        let mut s = Service::default();
        assert_eq!(
            s.call("soulseek.status", json!({}), &h).unwrap()["configured"],
            true
        );
        assert!(s.client.is_none());
        assert!(s.runtime.is_none());
        assert_eq!(h.count("secrets.get"), 0);
    }
    #[test]
    fn remote_paths_cannot_escape_flat_staging_destination() {
        for invalid in [
            "",
            "..",
            "dir/..",
            "dir/",
            "dir/evil?.mp3",
            "dir/evil\0.mp3",
        ] {
            assert!(validate_file(invalid).is_err());
        }
        assert!(validate_file("Music\\Artist\\song.mp3").is_ok());
        assert_eq!(basename("Music\\Artist\\song.mp3"), "song.mp3");
    }
}
