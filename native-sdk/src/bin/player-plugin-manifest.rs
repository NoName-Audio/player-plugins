//! Generate portable Android installation metadata from a source manifest.
use std::path::PathBuf;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let folder = PathBuf::from(args.next().ok_or("Missing plugin folder")?);
    let output = PathBuf::from(args.next().ok_or("Missing output path")?);
    let manifest = player_plugin_sdk::packaging::manifest(&folder)?;
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(
        output,
        serde_json::to_vec(
            &serde_json::json!({"manifest":manifest,"native":{"kind":"android-apk"}}),
        )?,
    )?;
    Ok(())
}
