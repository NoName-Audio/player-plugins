//! Resolve source-manifest assets when producing portable installation metadata.
use crate::{serde_json::Value, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use std::path::Path;

pub fn manifest(folder: &Path) -> Result<Value> {
    let mut manifest: Value = serde_json::from_slice(
        &std::fs::read(folder.join("plugin.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if let Some(image) = manifest.get("image").and_then(Value::as_str) {
        if !image.starts_with("https://")
            && !image.starts_with("http://")
            && !image.starts_with("data:")
        {
            let path = Path::new(image);
            if path.is_absolute() || image.contains(':') || image.contains('\\') {
                return Err("Image must be a relative file path or an HTTP(S) URL".into());
            }
            let mime = match path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase()
                .as_str()
            {
                "png" => "image/png",
                "jpg" | "jpeg" => "image/jpeg",
                "webp" => "image/webp",
                "gif" => "image/gif",
                _ => return Err("Local plugin image must be PNG, JPEG, WebP or GIF".into()),
            };
            let path = folder.join(path);
            if std::fs::metadata(&path).map_err(|e| e.to_string())?.len() > 256 * 1024 {
                return Err("Plugin image exceeds 256 KiB".into());
            }
            let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
            if bytes.is_empty() {
                return Err("Plugin image is empty".into());
            }
            manifest["image"] =
                Value::String(format!("data:{mime};base64,{}", STANDARD.encode(bytes)));
        }
    }
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_asset_is_embedded_without_rewriting_source_manifest() {
        let folder = std::env::temp_dir().join(format!("plugin-assets-{}", std::process::id()));
        std::fs::create_dir_all(folder.join("assets")).unwrap();
        let source = r#"{"image":"assets/icon.png"}"#;
        std::fs::write(folder.join("plugin.json"), source).unwrap();
        std::fs::write(folder.join("assets/icon.png"), b"png fixture").unwrap();
        assert_eq!(
            manifest(&folder).unwrap()["image"],
            "data:image/png;base64,cG5nIGZpeHR1cmU="
        );
        assert_eq!(
            std::fs::read_to_string(folder.join("plugin.json")).unwrap(),
            source
        );
        std::fs::remove_file(folder.join("assets/icon.png")).unwrap();
        assert!(manifest(&folder).is_err());
        std::fs::remove_dir_all(folder).unwrap();
    }
}
