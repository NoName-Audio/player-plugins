//! Build a portable JSON container; never execute plugin code.
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::PathBuf};
use player_plugin_sdk::packaging::{validate_runtime_name, MAX_LIBRARY_BYTES, MAX_PACKAGE_BYTES, MAX_RUNTIME_BYTES, MAX_TARGET_RUNTIME_BYTES};

fn payload(path: &str, maximum: u64) -> Result<(serde_json::Value, u64), Box<dyn std::error::Error>> {
    let length = std::fs::metadata(path)?.len();
    if length == 0 || length > maximum { return Err("Artifact is empty or exceeds its size limit".into()); }
    let bytes = std::fs::read(path)?;
    if bytes.is_empty() || bytes.len() as u64 > maximum { return Err("Artifact is empty or exceeds its size limit".into()); }
    let length = bytes.len() as u64;
    Ok((json!({"sha256":format!("{:x}",Sha256::digest(&bytes)),"data":STANDARD.encode(bytes)}), length))
}

fn pack() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let folder = PathBuf::from(args.next().ok_or("Usage: player-plugin-pack <plugin-folder> <output.player-plugin> <os-arch=library>... [--runtime target:name=path]...")?);
    let output = PathBuf::from(args.next().ok_or("Missing output path")?);
    let manifest = player_plugin_sdk::packaging::manifest(&folder)?;
    if manifest["apiVersion"] != 2 || manifest["id"].as_str().is_none_or(|id| id.is_empty()) {
        return Err("Only native API 2 packages with an ID can be packed".into());
    }
    let mut artifacts = BTreeMap::new();
    let mut runtimes = Vec::new();
    while let Some(artifact) = args.next() {
        if artifact == "--runtime" {
            runtimes.push(args.next().ok_or("Missing target:name=path after --runtime")?);
            continue;
        }
        let (target, path) = artifact.split_once('=').ok_or("Artifact must be os-arch=library-path")?;
        if !["macos-aarch64", "macos-x86_64", "linux-x86_64", "linux-aarch64", "windows-x86_64", "windows-aarch64"].contains(&target) {
            return Err("Unsupported native target".into());
        }
        let os = target.split('-').next().unwrap();
        if !manifest["platforms"].as_array().is_some_and(|p| p.iter().any(|v| v == os)) {
            return Err("Artifact OS is absent from manifest platforms".into());
        }
        if artifacts.insert(target.to_string(), payload(path, MAX_LIBRARY_BYTES)?.0).is_some() {
            return Err("Duplicate native target".into());
        }
    }
    if artifacts.is_empty() { return Err("At least one compiled native library is required".into()); }
    let mut totals = BTreeMap::<String, u64>::new();
    for runtime in runtimes {
        let (target, file) = runtime.split_once(':').ok_or("Runtime must be target:name=path")?;
        let (name, path) = file.split_once('=').ok_or("Runtime must be target:name=path")?;
        validate_runtime_name(name)?;
        let artifact = artifacts.get_mut(target).ok_or("Runtime target has no native library")?;
        let files = artifact.as_object_mut().unwrap().entry("files").or_insert_with(|| json!({})).as_object_mut().unwrap();
        if files.keys().any(|existing| existing.eq_ignore_ascii_case(name)) { return Err("Duplicate or case-colliding runtime filename".into()); }
        let (mut file, length) = payload(path, MAX_RUNTIME_BYTES)?;
        let total = totals.entry(target.to_string()).or_default();
        *total += length;
        if *total > MAX_TARGET_RUNTIME_BYTES { return Err("Target runtimes exceed 384 MiB".into()); }
        file["executable"] = json!(true);
        files.insert(name.to_string(), file);
    }
    let bytes = serde_json::to_vec(&json!({"manifest":manifest,"native":{"kind":"library","artifacts":artifacts}}))?;
    if bytes.len() as u64 > MAX_PACKAGE_BYTES { return Err("Native package exceeds 768 MiB".into()); }
    std::fs::write(&output, &bytes)?;
    println!("Created {} ({} bytes)", output.display(), bytes.len());
    Ok(())
}
fn main() {
    if let Err(error) = pack() { eprintln!("{error}"); std::process::exit(1); }
}
