//! Resolve source-manifest assets when producing portable installation metadata.
use crate::{serde_json::Value, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use std::path::Path;

pub fn manifest(folder: &Path) -> Result<Value> {
    let mut manifest: Value = serde_json::from_slice(
        &std::fs::read(folder.join("plugin.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    validate_attachments(&manifest)?;
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

/// Validate the additive API 2 attachment section without changing old contributions.
pub fn validate_attachments(manifest: &Value) -> Result<()> {
    let Some(attachments) = manifest.get("attachments") else {
        return Ok(());
    };
    let attachments = attachments
        .as_array()
        .ok_or("attachments must be an array")?;
    if attachments.len() > 32 {
        return Err("At most 32 attachments are allowed".into());
    }
    if !attachments.is_empty()
        && !manifest["permissions"]
            .as_array()
            .is_some_and(|permissions| permissions.iter().any(|permission| permission == "ui"))
    {
        return Err("Attachments require the ui permission".into());
    }
    let local_id = |value: &Value| {
        value.as_str().is_some_and(|id| {
            !id.is_empty()
                && id.len() <= 64
                && id
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
    };
    let mut ids = std::collections::HashSet::new();
    for attachment in attachments {
        let object = attachment
            .as_object()
            .ok_or("Attachment must be an object")?;
        if object.keys().any(|key| {
            !matches!(
                key.as_str(),
                "id" | "target" | "position" | "view" | "order"
            )
        }) {
            return Err("Unknown attachment field".into());
        }
        if !local_id(&attachment["id"]) || !local_id(&attachment["view"]) {
            return Err("Attachment id and view must be local IDs of 1–64 characters".into());
        }
        if !ids.insert(attachment["id"].as_str().unwrap()) {
            return Err("Duplicate attachment ID".into());
        }
        if !attachment["target"].as_str().is_some_and(|target| {
            !target.is_empty()
                && target.len() <= 128
                && target
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        }) {
            return Err("Invalid attachment target".into());
        }
        if !matches!(attachment["position"].as_str(), Some("before" | "after")) {
            return Err("Attachment position must be before or after".into());
        }
        if let Some(order) = object.get("order") {
            if !order
                .as_i64()
                .is_some_and(|order| i32::try_from(order).is_ok())
            {
                return Err("Attachment order must be a signed 32-bit integer".into());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn optional_attachments_are_strict_and_leave_contributions_untouched() {
        use serde_json::json;
        assert!(validate_attachments(&json!({"contributions":[{"id":"old"}]})).is_ok());
        let valid = json!({"permissions":["ui"],"attachments":[{"id":"album-info","target":"AlbumCard","position":"after","view":"album-info"}]});
        assert!(validate_attachments(&valid).is_ok());
        for (key, value) in [
            ("id", json!("Bad")),
            ("view", json!("")),
            ("target", json!("bad target")),
            ("position", json!("inside")),
            ("order", json!(1.5)),
            ("order", json!(2147483648_i64)),
            ("extra", json!(true)),
        ] {
            let mut invalid = valid.clone();
            invalid["attachments"][0][key] = value;
            assert!(validate_attachments(&invalid).is_err(), "{key}");
        }
        let mut invalid = valid.clone();
        invalid["attachments"]
            .as_array_mut()
            .unwrap()
            .push(valid["attachments"][0].clone());
        assert!(validate_attachments(&invalid).is_err());
        invalid = valid.clone();
        invalid["permissions"] = json!([]);
        assert!(validate_attachments(&invalid).is_err());
        assert!(validate_attachments(&json!({"attachments":null})).is_err());
    }
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
