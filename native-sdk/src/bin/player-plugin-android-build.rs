//! Cargo/NDK helper called by the standalone APK's Gradle task.
use std::{path::PathBuf, process::Command};
fn build() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let folder =
        PathBuf::from(args.next().ok_or("Missing Android plugin directory")?).canonicalize()?;
    let abis = args.next().unwrap_or_else(|| "arm64-v8a".into());
    let library = args
        .next()
        .unwrap_or_else(|| "player_native_example".into());
    if !library
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err("Invalid library name".into());
    }
    let sdk = PathBuf::from(
        std::env::var("ANDROID_HOME").or_else(|_| std::env::var("ANDROID_SDK_ROOT"))?,
    );
    let ndk = if let Ok(ndk) = std::env::var("ANDROID_NDK_HOME") {
        PathBuf::from(ndk)
    } else {
        std::fs::read_dir(sdk.join("ndk"))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .max_by_key(|path| {
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .split('.')
                    .map(|p| p.parse::<u32>().unwrap_or(0))
                    .collect::<Vec<_>>()
            })
            .ok_or("No installed Android NDK")?
    };
    let host = match std::env::consts::OS {
        "macos" => "darwin-x86_64",
        "linux" => "linux-x86_64",
        "windows" => "windows-x86_64",
        _ => return Err("Unsupported NDK host".into()),
    };
    for abi in abis.split(',') {
        let (target, clang) = match abi {
            "arm64-v8a" => ("aarch64-linux-android", "aarch64-linux-android"),
            "armeabi-v7a" => ("armv7-linux-androideabi", "armv7a-linux-androideabi"),
            "x86_64" => ("x86_64-linux-android", "x86_64-linux-android"),
            "x86" => ("i686-linux-android", "i686-linux-android"),
            _ => return Err("Unsupported Android ABI".into()),
        };
        let linker = ndk
            .join("toolchains/llvm/prebuilt")
            .join(host)
            .join("bin")
            .join(format!(
                "{clang}26-clang{}",
                if cfg!(windows) { ".cmd" } else { "" }
            ));
        if !linker.exists() {
            return Err(format!("NDK compiler not found: {}", linker.display()).into());
        }
        let output = folder.join("native/target");
        let status = Command::new("cargo")
            .args(["build", "--locked", "--release", "--manifest-path"])
            .arg(folder.join("native/Cargo.toml"))
            .args(["--target", target])
            .env("CARGO_TARGET_DIR", &output)
            .env(
                format!(
                    "CARGO_TARGET_{}_LINKER",
                    target.to_uppercase().replace('-', "_")
                ),
                &linker,
            )
            .status()?;
        if !status.success() {
            return Err("Android Rust compilation failed".into());
        }
        let destination = folder.join("build/generated/jniLibs").join(abi);
        std::fs::create_dir_all(&destination)?;
        std::fs::copy(
            output.join(target).join(format!("release/lib{library}.so")),
            destination.join(format!("lib{library}.so")),
        )?;
    }
    Ok(())
}
fn main() {
    if let Err(error) = build() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
