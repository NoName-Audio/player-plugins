//! Deliberately structured diagnostics: never serialize protocol payloads or errors.
use sha2::{Digest, Sha256};
use std::{fs::{self, OpenOptions}, io::Write, path::{Path, PathBuf}, time::{SystemTime, UNIX_EPOCH}};

const MAX_BYTES: u64 = 512 * 1024;
#[derive(Default)]
pub struct Diagnostics { path: PathBuf }
impl Diagnostics {
    pub fn open(root: &Path) -> std::io::Result<Self> {
        let directory = root.join("diagnostics");
        fs::create_dir_all(&directory)?;
        Ok(Self { path: directory.join("soulseek.jsonl") })
    }
    pub fn record(&self, event: &'static str, resource: Option<&str>, bytes: Option<u64>, total: Option<u64>, queue: Option<u64>) {
        self.record_with_category(event, resource, bytes, total, queue, None);
    }
    pub fn record_with_category(&self, event: &'static str, resource: Option<&str>, bytes: Option<u64>, total: Option<u64>, queue: Option<u64>, category: Option<&'static str>) {
        if self.path.as_os_str().is_empty() { return; }
        let mut value = serde_json::json!({"timeMs":SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis(),"event":event});
        if let Some(category) = category { value["failureCategory"] = category.into(); }
        if let Some(id) = resource { value["resource"] = serde_json::json!(format!("{:x}", Sha256::digest(id.as_bytes()))); }
        for (key, field) in [("bytes",bytes),("totalBytes",total),("queuePosition",queue)] {
            if let Some(field) = field { value[key] = field.into(); }
        }
        let mut line = value.to_string(); line.push('\n');
        // One archive; rename replaces the previous archive on supported platforms.
        if self.path.metadata().is_ok_and(|m| m.len() + line.len() as u64 > MAX_BYTES) {
            if fs::rename(&self.path, self.path.with_extension("jsonl.1")).is_err() { return; }
        }
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&self.path) {
            let _ = file.write_all(line.as_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn redacts_resource_and_rotates_with_bounded_files() {
        let root = std::env::temp_dir().join(format!("soulseek-diagnostics-{}-{}",std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
        let log = Diagnostics::open(&root).unwrap();
        let secret = "[\"private-user\",\"/private/music.flac\"]";
        log.record_with_category("queued_remote",Some(secret),Some(12),Some(100),Some(3),Some("connection_reset"));
        let line = fs::read_to_string(&log.path).unwrap();
        assert!(!line.contains("private"));
        let value: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(value["queuePosition"],3);
        assert_eq!(value["failureCategory"], "connection_reset");
        assert_eq!(value["resource"].as_str().unwrap().len(),64);
        for _ in 0..6000 { log.record("progress",Some(secret),Some(12),Some(100),None); }
        assert!(log.path.metadata().unwrap().len() <= MAX_BYTES);
        assert!(log.path.with_extension("jsonl.1").metadata().unwrap().len() <= MAX_BYTES);
        assert_eq!(fs::read_dir(root.join("diagnostics")).unwrap().count(),2);
        fs::remove_dir_all(root).unwrap();
    }
}
