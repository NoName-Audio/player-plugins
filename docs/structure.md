# Repository structure

[Documentation](README.md) · [Development](development.md)

```text
player-plugins/
├── Cargo.toml                         # Rust workspace and shared dependencies
├── Cargo.lock                         # pinned workspace dependencies
├── README.md                          # overview and quick start
├── catalog.json                       # public native package listing, schema 2
├── .gitignore                         # local build/package/cache exclusions
├── .github/workflows/check.yml        # Rust tests and builds on macOS/Linux/Windows
├── native-sdk/
│   ├── Cargo.toml                     # shared SDK crate
│   └── src/
│       ├── lib.rs                     # NativePlugin, Host, C ABI, export macro
│       └── bin/
│           ├── player-plugin-pack.rs  # compiled libraries → .player-plugin
│           └── player-plugin-android-build.rs # Cargo/NDK helper used by Gradle
├── native-android-sdk/                # reusable JNI host callbacks and export macro
├── tools/                             # verified release merger and Python tests
├── .github/workflows/release.yml      # independent binary releases and catalog publication
├── discord-rpc/
│   ├── README.md                      # behavior and file map
│   ├── Cargo.toml                     # rlib/cdylib; optional ffi exports
│   ├── plugin.json                    # metadata, permissions, settings contribution
│   ├── src/adapter.rs                 # package-owned Discord IPC worker
│   ├── src/lib.rs                     # native playback handler and UI
│   └── assets/                        # Discord SVG/PNG and attribution README
├── soulseek/
│   ├── README.md                      # behavior and file map
│   ├── Cargo.toml                     # native package crate
│   ├── plugin.json                    # settings/search/menu/widget contributions
│   ├── src/service.rs                 # own runtime, client and transfer workers
│   ├── vendor/soulseek-rs-lib/         # modified protocol client
│   ├── android/                       # independent Kotlin/JNI Soulseek APK
│   ├── src/lib.rs                     # UI, search, transfer intentions, preview/import
│   └── assets/                        # Soulseek PNG and attribution README
├── examples/
│   ├── hello/
│   │   ├── README.md
│   │   ├── Cargo.toml                 # desktop native library crate
│   │   ├── plugin.json                # external package example.hello
│   │   └── src/lib.rs                 # native settings, page, and playback widget
│   └── android-native/
│       ├── README.md
│       ├── plugin.json                # API 2 manifest included in the APK
│       ├── build.gradle.kts           # assets, Rust build, APK packaging
│       ├── settings.gradle.kts        # Gradle plugins and repositories
│       ├── gradle.properties
│       ├── proguard-rules.pro         # reflection/JNI entrypoint preservation
│       ├── src/main/
│       │   ├── AndroidManifest.xml    # discovery metadata, native API 2
│       │   └── java/dev/verncat/player/example/nativeplugin/NativePlugin.kt
│       └── native/
│           ├── Cargo.toml             # JNI library, uses the workspace lockfile
│           └── src/lib.rs             # native lifecycle/UI/actions and JNI bridge
├── docs/                              # index, structure, development, SDK, catalog, tooling
├── gradlew / gradlew.bat               # standalone Android Gradle wrapper
└── gradle/wrapper/                     # wrapper JAR and pinned Gradle distribution
```

## Package sources

A Rust desktop package has `plugin.json`, `Cargo.toml`, and `src/lib.rs`.
The manifest's ID must match the ID exported by its compiled library. `assets/`
contains image sources and attribution; relative asset paths are not supported in
manifest `image`, so use an HTTP(S) URL or an embedded raster data URL.

The default `ffi` feature exports the desktop C ABI from each `cdylib`.
Android wrappers use the same Rust package as an `rlib` with FFI exports disabled.
Player links neither provider crate.

New crates must be added to the root workspace's `members`. Keep package IDs stable
across updates. Source directories alone are not installable: publish compiled libraries
inside a native package, or install an Android APK.

## Generated files

| Output | Location |
| --- | --- |
| Host Rust libraries and tools | `target/` |
| Packaged native extension | Explicit `.player-plugin` output path |
| Android Rust target builds | `examples/android-native/native/target/` |
| Copied Android libraries | `examples/android-native/build/generated/jniLibs/<abi>/` |
| APK package metadata | `examples/android-native/build/generated/pluginAssets/player-plugin.json` |
| Debug APK | `examples/android-native/build/outputs/apk/debug/android-native-debug.apk` |
| Gradle caches / local SDK settings | `.gradle/`, `local.properties` |

Generated outputs are ignored by Git. The root `Cargo.lock` and Gradle wrapper are tracked.

## Boundary with Player

Player owns loading, package approval, registry/settings/secrets, UI rendering,
playback coordination, and the native capability gateway. Provider IPC, network clients, and background workers live entirely in the
independently built packages.
The APK example's Rust code belongs to the separately installed APK.

In Player, this repository is the pinned `plugins/` submodule.
See [Player integration](tooling.md#player-integration) for updating it.
