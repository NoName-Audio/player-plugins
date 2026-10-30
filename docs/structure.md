# Repository structure

[Documentation](README.md) · [Plugin development](development.md)

```text
player-plugins/
├── README.md                     # entry point and quick start
├── catalog.json                  # public plugin listing
├── package.json                  # pnpm commands and tooling dependencies
├── pnpm-lock.yaml                # pinned TypeScript version
├── sdk.d.ts                      # global API 1 declarations
├── tsconfig.sdk.json             # SDK declaration checks
├── .gitignore                    # dependency and build output exclusions
├── .github/workflows/check.yml   # package and SDK checks on push / pull request
├── docs/                         # shared guides
│   ├── README.md                 # documentation index
│   ├── structure.md              # this document
│   ├── development.md            # package creation and manifest format
│   ├── sdk.md                    # API, lifecycle, and declarative UI
│   ├── catalog.md                # publishing the plugin listing
│   └── tooling.md                # commands, checks, and Player integration
├── discord-rpc/                  # bundled Player package
│   ├── README.md                 # behavior and package file map
│   ├── plugin.json               # ID, platforms, permissions, and settings section
│   ├── main.js                   # playback events → host Discord API
│   └── assets/
│       ├── README.md             # logo attribution and PNG generation
│       ├── discord.svg           # vector source
│       └── discord.png           # logo embedded in the manifest as a data URL
├── soulseek/                     # bundled Player package
│   ├── README.md                 # search, downloads, preview, and replacement
│   ├── plugin.json               # permissions and settings/search/track-menu contributions
│   ├── main.js                   # UI and native operation coordination
│   └── assets/
│       ├── README.md             # logo attribution
│       └── soulseek.png          # logo embedded in the manifest as a data URL
├── examples/
│   ├── hello/
│   │   ├── README.md             # running the example and expected behavior
│   │   ├── plugin.json           # regular external package example.hello
│   │   └── main.js               # settings, a page, and playback events
│   └── android-native/
│       ├── README.md             # building, connecting, and testing the APK
│       ├── plugin.json           # plugin manifest embedded in the APK
│       ├── main.js               # UI and ctx.android.call()
│       ├── build.gradle.kts      # APK, JSON assets, and Rust JNI build
│       ├── settings.gradle.kts   # Android/Kotlin Gradle plugins and repositories
│       ├── gradle.properties     # Gradle and AndroidX settings
│       ├── proguard-rules.pro    # reflection/JNI entrypoint preservation
│       ├── src/main/
│       │   ├── AndroidManifest.xml
│       │   └── java/dev/verncat/player/example/nativeplugin/NativePlugin.kt
│       └── native/
│           ├── Cargo.toml        # player_native_example Rust cdylib
│           ├── Cargo.lock        # pinned JNI/JSON dependencies
│           └── src/lib.rs        # JNI entrypoint, echo, and unit test
├── scripts/
│   ├── package-plugin.mjs        # plugin.json + main.js → .player-plugin
│   └── build-android-plugin.mjs  # Cargo + NDK → .so for selected ABIs
├── tests/packages.test.mjs       # packaging and catalog consistency checks
├── gradlew / gradlew.bat         # Gradle wrapper for the Android example
└── gradle/wrapper/
    ├── gradle-wrapper.jar
    └── gradle-wrapper.properties # pinned Gradle distribution
```

## Regular package layout

A package needs at least `plugin.json` and `main.js` in the same directory.
The manifest describes the package; `main.js` contains all executable code
and one call to `registerPlugin`. See [package creation](development.md).

A package's `README.md` explains its behavior. `assets/` holds image sources
and attribution. The packer only reads the two required files: images and other
resources are not automatically included. Relative paths in `image` are unsupported;
use HTTP(S) or a data URL. Replacing a PNG in `assets/` does not update the image
already embedded in `plugin.json`.

Create each new plugin in a separate directory; `examples/` is for sample packages.
The directory name and plugin ID are separate: the manifest defines the ID, which
must remain stable across updates. Adding a directory does not automatically add
an entry to the [catalog](catalog.md).

## Sources and generated files

| Output | Location | Storage |
| --- | --- | --- |
| Installed npm dependencies | `node_modules/` | Ignored by Git |
| Packaged JS plugin | Explicit output or `<folder>/<id>-<version>.player-plugin` | Ignored by Git |
| Rust Android build | `examples/android-native/native/target/` | Ignored by Git |
| Copied JNI libraries | `examples/android-native/build/generated/jniLibs/<abi>/` | Ignored by Git |
| APK manifest/script JSON | `examples/android-native/build/generated/pluginAssets/player-plugin.json` | Generated by Gradle |
| Debug APK | `examples/android-native/build/outputs/apk/debug/android-native-debug.apk` | Ignored by Git |
| Gradle cache / SDK path | `.gradle/`, `local.properties` | Ignored by Git |

`Cargo.lock`, `pnpm-lock.yaml`, and the Gradle wrapper are tracked for reproducible builds.

## Boundary with Player

This repository holds packages. Player owns the Worker runtime, UI renderer,
manifest validation, installation, storage, and native gateway. Player's `src/`
and `src-tauri/` directories therefore live in the Player repository.
Rust in `examples/android-native/native/` belongs to the standalone APK;
the bundled Discord/Soulseek Rust adapters live in Player.

Player includes this repository as the `plugins/` submodule. For example,
`player-plugins/soulseek/main.js` is available there as `Player/plugins/soulseek/main.js`.
See [tooling and delivery](tooling.md#player-integration) for updating the pinned commit.
