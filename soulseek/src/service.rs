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
const COVER_BASENAME_PRIORITY: &[&str] = &["cover", "folder", "front", "album", "art"];
#[derive(Default)]
pub struct Service {
    runtime: Option<tokio::runtime::Runtime>,
    root: PathBuf,
    client: Option<Client>,
    config: Option<(String, String)>,
    previews: HashMap<String, (PathBuf, Arc<Mutex<String>>)>,
    jobs: HashMap<String, DownloadCanceller>,
    events: Arc<Mutex<BTreeMap<String, Value>>>,
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
        self.runtime()?
            .block_on(client.connect())
            .map_err(|e| e.to_string())?;
        Ok(client)
    }
    fn cancel_all(&mut self) {
        for (_, job) in self.jobs.drain() {
            job.cancel();
        }
    }
    pub fn close(&mut self) {
        self.cancel_all();
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
        let user = request["username"]
            .as_str()
            .ok_or("Missing peer username")?;
        let file = request["coverFilename"]
            .as_str()
            .ok_or("Missing cover filename")?;
        let size = request["coverSize"].as_u64().ok_or("Missing cover size")?;
        if size == 0 || size > 32 * 1024 * 1024 {
            return Err("Invalid cover size".into());
        }
        self.start(
            SoulseekDownloadRequest {
                username: user.into(),
                filename: file.into(),
                size,
            },
            Some(args["id"].clone()),
            host,
        )
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
        self.runtime()?.spawn(async move {
   let mut terminal=false;
   while let Some(status)=handle.recv().await{
    let mut event=json!({"username":request.username,"filename":request.filename,"basename":basename(&request.filename),"localPath":path,"totalBytes":request.size});
    let state_name=match status {
     DownloadStatus::QueuedLocally=>"queued_local",
     DownloadStatus::QueuedRemotely{place}=>{event["queuePosition"]=json!(place);"queued_remote"},
     DownloadStatus::InProgress{bytes_downloaded,total_bytes,speed_bytes_per_sec}=>{event["bytesDownloaded"]=json!(bytes_downloaded);event["totalBytes"]=json!(total_bytes);event["speedBytesPerSec"]=json!(speed_bytes_per_sec);"progress"},
     DownloadStatus::Completed=>{terminal=true;"completed"},DownloadStatus::Failed=>{terminal=true;event["error"]=json!("Download failed");"failed"},
     DownloadStatus::TimedOut=>{terminal=true;event["error"]=json!("Download timed out");"timed_out"},DownloadStatus::Cancelled=>{terminal=true;"cancelled"},
    };
    event["state"]=json!(state_name);*state.lock().unwrap()=if terminal{state_name}else{"active"}.into();
    if let Some(cover_id)=&cover {
     if terminal{events.lock().unwrap().insert(format!("cover:{id}"),json!({"topic":"soulseek-cover","payload":{"id":cover_id,"path":if state_name=="completed"{Some(path.clone())}else{None},"result":null}}));}
    }else{events.lock().unwrap().insert(format!("transfer:{id}"),json!({"topic":"soulseek-preview","payload":event}));}
    if terminal{break;}
   }
   if !terminal{*state.lock().unwrap()="failed".into();let value=if let Some(cover_id)=cover{json!({"topic":"soulseek-cover","payload":{"id":cover_id,"result":null}})}else{json!({"topic":"soulseek-preview","payload":{"username":request.username,"filename":request.filename,"localPath":path,"state":"failed","error":"Transfer ended unexpectedly"}})};events.lock().unwrap().insert(format!("ended:{id}"),value);}
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
    pub cover_filename: Option<String>,
    pub cover_size: Option<u64>,
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

struct PendingSearchFile {
    result: SoulseekSearchResult,
    parent: Option<String>,
}

struct PendingCoverFile {
    filename: String,
    size: u64,
    priority: usize,
}

fn flatten_search_results(results: Vec<soulseek_rs::SearchResult>) -> Vec<SoulseekSearchResult> {
    let mut audio_files = Vec::new();
    let mut covers: HashMap<(String, String), PendingCoverFile> = HashMap::new();

    for result in results {
        let free_upload_slots = result.slots;
        let peer_speed = result.speed;
        for file in result.files {
            let mapped = map_search_file(file, free_upload_slots, peer_speed);
            if let (Some(parent), Some(priority)) = (
                mapped.parent.as_ref(),
                cover_file_priority(&mapped.result.basename, mapped.result.extension.as_deref()),
            ) {
                let key = (mapped.result.username.clone(), parent.clone());
                let candidate = PendingCoverFile {
                    filename: mapped.result.filename.clone(),
                    size: mapped.result.size,
                    priority,
                };
                match covers.get(&key) {
                    Some(current) if current.priority <= candidate.priority => {}
                    _ => {
                        covers.insert(key, candidate);
                    }
                }
                continue;
            }

            if is_audio_result(&mapped.result) {
                audio_files.push(mapped);
            }
        }
    }

    let mut files = Vec::with_capacity(audio_files.len());
    for mut mapped in audio_files {
        if let Some(parent) = mapped.parent.as_ref() {
            if let Some(cover) = covers.get(&(mapped.result.username.clone(), parent.clone())) {
                mapped.result.cover_filename = Some(cover.filename.clone());
                mapped.result.cover_size = Some(cover.size);
            }
        }
        files.push(mapped.result);
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

fn map_search_file(file: File, free_upload_slots: u8, peer_speed: u32) -> PendingSearchFile {
    let basename = file.name.filename().to_string();
    let filename = file.name.to_string();
    let extension = Path::new(&basename)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());

    PendingSearchFile {
        parent: soulseek_parent(&filename),
        result: SoulseekSearchResult {
            username: file.username,
            filename,
            basename,
            cover_filename: None,
            cover_size: None,
            size: file.size,
            bitrate: file.attributes.bitrate,
            duration: file.attributes.duration,
            sample_rate: file.attributes.sample_rate,
            bit_depth: file.attributes.bit_depth,
            vbr: file.attributes.vbr,
            peer_speed,
            free_upload_slots,
            extension,
        },
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

fn cover_file_priority(basename: &str, extension: Option<&str>) -> Option<usize> {
    let extension = extension?;
    if !COVER_EXTENSIONS.contains(&extension) {
        return None;
    }

    let stem = Path::new(basename)
        .file_stem()
        .and_then(|value| value.to_str())?
        .to_ascii_lowercase();

    COVER_BASENAME_PRIORITY
        .iter()
        .position(|candidate| *candidate == stem)
}

fn soulseek_parent(filename: &str) -> Option<String> {
    let mut segments: Vec<&str> = filename
        .split(['\\', '/'])
        .filter(|segment| !segment.is_empty())
        .collect();
    if segments.len() <= 1 {
        return None;
    }
    segments.pop();
    Some(segments.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use player_plugin_sdk::testing::TestHost;
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
