# Native tooling and delivery

[Documentation](README.md) · [File structure](structure.md)

All commands below run from `player-plugins` unless explicitly marked **Player**.
Node.js, pnpm, and a script bundler are not required for this repository.

| Command | Purpose |
| --- | --- |
| `cargo test --workspace --locked` | SDK, package UI/actions, transfer intentions, and JNI-core tests |
| `cargo build --workspace --locked` | Build libraries and native CLI tools for the host |
| `cargo build --release -p player-hello-plugin` | Build the desktop example |
| `cargo run -p player-plugin-sdk --bin player-plugin-pack -- <folder> <output> <os-arch=library>...` | Package compiled native libraries |
| `./gradlew -p examples/android-native assembleDebug` | Build the standalone Android APK |

The packer is [player-plugin-pack.rs](../native-sdk/src/bin/player-plugin-pack.rs).
It reads `plugin.json`, embeds binary bytes and SHA-256 by target, and writes
`{manifest, native: {kind: "library", artifacts}}`. It does not compile libraries,
execute their constructors, or guarantee full host manifest validation.

The Android helper is [player-plugin-android-build.rs](../native-sdk/src/bin/player-plugin-android-build.rs).
Gradle runs it through Cargo. It selects the NDK from `ANDROID_NDK_HOME` or the SDK
from `ANDROID_HOME` / `ANDROID_SDK_ROOT`, builds with `--locked --release`, and copies
JNI libraries into Gradle's generated output. The workspace root lockfile is used.

## Checks

[check.yml](../.github/workflows/check.yml) tests and builds the Rust workspace on
macOS, Linux, and Windows. Android APK/device integration is separate.
The SDK's `testing` feature provides a mock `Host` for native package tests.

In the **Player** repository, native host checks include:

```sh
pnpm test:plugins
pnpm test:plugin-sdk
pnpm exec vue-tsc --noEmit
cargo test --manifest-path src-tauri/Cargo.toml --lib
```

Host tests compile and load a real Hello shared library through C ABI 1, verify host
callbacks, reject stale identities, and test frontend IPC/permission routing. Live
Discord/Soulseek services and signed release builds require their platform checks.

## Player integration

`Player/plugins/` is a pinned submodule of this repository. A sibling checkout at
`../player-plugins` is independent. From the **Player** root:

```sh
git submodule update --init -- plugins
```

To update bundled code after publishing a tested plugin commit:

```sh
git -C plugins fetch origin main
git -C plugins checkout --detach origin/main
git add plugins
git commit -m "Update bundled native plugins"
```

Player links Discord/Soulseek Rust crates with default FFI exports disabled. External
plugins use the dynamic loader instead. Release CI must fetch the pinned submodule
before Cargo resolves these path dependencies.

On macOS, hardened release binaries retain their normal library signature validation.
This implementation does not add an entitlement disabling it. Native artifacts must
meet those signing requirements. See Apple's
[library validation documentation](https://developer.apple.com/documentation/bundleresources/entitlements/com.apple.security.cs.disable-library-validation).
