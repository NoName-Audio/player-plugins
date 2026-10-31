//! Build a portable JSON container with native libraries; never execute plugin code.
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::PathBuf};
fn pack() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let folder = PathBuf::from(args.next().ok_or(
        "Usage: player-plugin-pack <plugin-folder> <output.player-plugin> <os-arch=library>...",
    )?);
    let output = PathBuf::from(args.next().ok_or("Missing output path")?);
    let manifest: Value = serde_json::from_slice(&std::fs::read(folder.join("plugin.json"))?)?;
    if manifest["apiVersion"] != 2 || manifest["id"].as_str().is_none_or(|id| id.is_empty()) {
        return Err("Only native API 2 packages with an ID can be packed".into());
    }
    let mut artifacts = BTreeMap::new();
    for artifact in args {
        let (target, path) = artifact
            .split_once('=')
            .ok_or("Artifact must be os-arch=library-path")?;
        if ![
            "macos-aarch64",
            "macos-x86_64",
            "linux-x86_64",
            "linux-aarch64",
            "windows-x86_64",
            "windows-aarch64",
        ]
        .contains(&target)
        {
            return Err("Unsupported native target".into());
        }
        let os = target.split('-').next().unwrap();
        if !manifest["platforms"]
            .as_array()
            .is_some_and(|p| p.iter().any(|v| v == os))
        {
            return Err("Artifact OS is absent from manifest platforms".into());
        }
        if std::fs::metadata(path)?.len() > 64 * 1024 * 1024 {
            return Err("Native library exceeds 64 MiB".into());
        }
        let bytes = std::fs::read(path)?;
        if bytes.is_empty() {
            return Err("Native library is empty".into());
        }
        if artifacts.insert(target.to_string(),json!({"sha256":format!("{:x}",Sha256::digest(&bytes)),"data":STANDARD.encode(bytes)})).is_some(){return Err("Duplicate native target".into());}
    }
    if artifacts.is_empty() {
        return Err("At least one compiled native library is required".into());
    }
    let bytes = serde_json::to_vec(
        &json!({"manifest":manifest,"native":{"kind":"library","artifacts":artifacts}}),
    )?;
    if bytes.len() > 96 * 1024 * 1024 {
        return Err("Native package exceeds 96 MiB".into());
    }
    std::fs::write(&output, &bytes)?;
    println!("Created {} ({} bytes)", output.display(), bytes.len());
    Ok(())
}
fn main() {
    if let Err(error) = pack() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
