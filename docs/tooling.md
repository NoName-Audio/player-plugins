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
| `cargo run -p player-plugin-sdk --bin player-plugin-manifest -- <folder> <output>` | Generate APK metadata, resolving local image paths |
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

Player depends only on `native-sdk`; provider crates and their network/IPC dependencies
are absent from its Cargo graph. Updating this submodule updates the shared SDK source,
not installed provider code. Install provider updates through the normal package flow.

## Release pipeline

Push a `native-v*` tag to run [release.yml](../.github/workflows/release.yml).
It builds both desktop packages for six OS/CPU targets and a standalone four-ABI
Soulseek Android APK. [merge-packages.py](../tools/merge-packages.py) verifies binary
hashes and matching manifests before merging target packages. Release assets are
uploaded before the generated catalog is committed to main; failed builds leave the
catalog unchanged. Run `python3 -m unittest discover -s tools -p 'test_*.py'` locally.

The APK is currently an experimental debug-signed build. CI debug keys are ephemeral;
updates from another signer may require uninstall/reinstall. Production APK updates
require a persistent release signing key configured separately. No Player rebuild is
needed for either desktop package installation or APK discovery.

On macOS, hardened release binaries retain their normal library signature validation.
This implementation does not add an entitlement disabling it. Native artifacts must
meet those signing requirements. See Apple's
[library validation documentation](https://developer.apple.com/documentation/bundleresources/entitlements/com.apple.security.cs.disable-library-validation).
