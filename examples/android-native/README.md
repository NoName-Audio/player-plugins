# Android native APK example

Standalone Kotlin/Rust package `example.android-native`. All lifecycle, UI, and actions
are implemented in native Rust; Kotlin provides discovery/reflection and the JNI bridge.
There is no JavaScript asset or Worker.

| File | Purpose |
| --- | --- |
| [plugin.json](plugin.json) | API 2 manifest with `ui`, `settings`, and `native.android` |
| [native/src/lib.rs](native/src/lib.rs) | NativePlugin implementation, UI, settings callback, and JNI entrypoints |
| [native/Cargo.toml](native/Cargo.toml) | JNI library using the root workspace lockfile |
| [NativePlugin.kt](src/main/java/dev/verncat/player/example/nativeplugin/NativePlugin.kt) | Native instance lifecycle and host callback bridge |
| [AndroidManifest.xml](src/main/AndroidManifest.xml) | Discovery marker, API 2, plugin ID, entry class |
| [build.gradle.kts](build.gradle.kts) | Rust build, package metadata, APK |
| [settings.gradle.kts](settings.gradle.kts) | Gradle plugins and repositories |
| [proguard-rules.pro](proguard-rules.pro) | Native/reflection entrypoint preservation |

## Build

Requires Rust, the selected Android Rust targets, Android SDK/NDK, and JDK 17+.
From the repository root:

```sh
export ANDROID_HOME="$HOME/Library/Android/sdk"
rustup target add aarch64-linux-android
./gradlew -p examples/android-native assembleDebug
```

Gradle runs the Rust SDK's `player-plugin-android-build` tool, then includes the JNI
library and `{manifest, native: {kind: "android-apk"}}` metadata in the APK.
Output: `examples/android-native/build/outputs/apk/debug/android-native-debug.apk`.
Debug builds use a local debug signing key; publishing requires a persistent release key.

Select additional ABIs after installing their Rust targets:

```sh
./gradlew -p examples/android-native assembleDebug -PpluginAbis=arm64-v8a,x86_64
```

## Connect and call

Install the current native-API-2 Player Android app and the example APK. In Player:

1. Open Settings → Plugins → Find APK plugins.
2. Review the native package and signing identity, install its connection, then enable it.
3. The page renders `Rust via JNI` and its architecture from native code.
4. Enter a message and select Call Rust. The counter increments and the message is
   saved through the host's settings API.

Path: Player native dispatcher → Kotlin `call(method, argsJson, hostCallback)` →
Rust `NativePlugin::call`. Host calls travel back through Kotlin and Player's JNI bridge,
using an invocation-scoped token. UI descriptions return as JSON; no plugin code runs
in the webview. The APK executes inside Player's process with Player's system access.

The loader derives package/class and file/certificate SHA-256 bindings from the
installed APK. Do not add those hashes manually to the source manifest. Updating a
loaded JNI library requires a Player process restart, rescan, and approval.
Removing a connection does not uninstall the Android APK.

## Verify

```sh
cargo test -p player-native-example
```

The Player repository contains `ApkPluginsTest`, covering real APK pinning, native UI,
actions, host callbacks, and lifecycle generation checks. Build/install Player and the
example APK first; then, from **Player** `src-tauri/gen/android`:

```sh
./gradlew :app:assembleArm64DebugAndroidTest -x :app:rustBuildArm64Debug
adb -s emulator-5554 install -r app/build/outputs/apk/androidTest/arm64/debug/app-arm64-debug-androidTest.apk
adb -s emulator-5554 shell am instrument -w -e class dev.verncat.player.ApkPluginsTest dev.verncat.player.test/androidx.test.runner.AndroidJUnitRunner
```

Skipping rustBuild is valid only after a normal Player build; otherwise Gradle requires
a running Tauri build server. Device instrumentation is separate from workspace tests.

[Documentation](../../docs/README.md) · [Native SDK](../../docs/sdk.md)
