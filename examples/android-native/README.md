# Android APK / Rust example

A standalone native plugin experiment. Its UI runs in the standard Worker;
the APK contains a Kotlin entrypoint and a Rust JNI library. The APK is built
separately from Player. Contract and limitations:
[Android native scope](https://github.com/verncat/player/blob/feat/plugins/docs/plugins/scope/android-native-scope.md).

## Files and call path

| File | Purpose |
| --- | --- |
| [plugin.json](plugin.json) | Metadata, `native.android`, and the `home` page |
| [main.js](main.js) | UI tree and `ctx.android.call('echo', {message})` |
| [src/main/AndroidManifest.xml](src/main/AndroidManifest.xml) | Discovery marker, plugin ID, API version, and entry class name |
| [NativePlugin.kt](src/main/java/dev/verncat/player/example/nativeplugin/NativePlugin.kt) | Reflection ABI, `.so` loading, and JSON requests through JNI |
| [native/src/lib.rs](native/src/lib.rs) | JNI entrypoint, `echo`, a call counter, and a unit test |
| [native/Cargo.toml](native/Cargo.toml) / [Cargo.lock](native/Cargo.lock) | Rust cdylib and pinned dependencies |
| [build.gradle.kts](build.gradle.kts) | JSON asset generation, Rust build, and APK packaging |
| [settings.gradle.kts](settings.gradle.kts) | Android/Kotlin Gradle plugins and repositories |
| [gradle.properties](gradle.properties) / [proguard-rules.pro](proguard-rules.pro) | Build settings and entrypoint preservation |

Call path: Worker `main.js` → Player gateway → Kotlin `NativePlugin.call()` →
JNI `nativeCall()` → Rust `echo`. The APK code runs inside Player's process;
native code is not isolated by the Worker. Arguments and results cross as JSON.

Gradle builds `build/generated/pluginAssets/player-plugin.json` from the manifest
and JS. Player's loader creates the installed APK binding with file and certificate
SHA-256 hashes; do not add those values manually to the source `plugin.json`.

## Build

Requires Android SDK/NDK, JDK 17+, a Rust Android target, and Node.js.
Run from the player-plugins root:

```sh
export ANDROID_HOME="$HOME/Library/Android/sdk" # adjust to your SDK location
rustup target add aarch64-linux-android
pnpm plugins:android:build
```

Output: `examples/android-native/build/outputs/apk/debug/android-native-debug.apk`.
The debug APK uses the local debug signing key. For publishing, configure release
signing with your own persistent key; the `.player-plugin` packer does not install APKs.
Cargo.lock is included. Rust builds through NDK clang without a cargo-ndk dependency.

Select other ABIs through a Gradle property after installing their Rust targets:

```sh
./gradlew -p examples/android-native assembleDebug -PpluginAbis=arm64-v8a,x86_64
```

## Connect

In the **Player** repository, build the Android app with `pnpm android:build`.
For Player's native Opus dependency, set `ANDROID_NDK_HOME` to an installed NDK
version under `$ANDROID_HOME/ndk`. Install Player and the example APK using
the system installer or adb, then:

1. Open Settings → Plugins → Find APK plugins.
2. Select Native example → Review connection; review its details and native-code permission.
3. Choose Install and grant these permissions, then enable the plugin.
4. The Native example page should show `Rust via JNI` and the architecture.
5. Enter a message and select Call Rust; the call counter should increase.

When updating the APK, disable its connection, rescan, and approve the new version.
If its `.so` has already been loaded, restart the Player process completely.
Removing the connection from Player does not uninstall the Android APK.

## Verify

```sh
cargo test --manifest-path examples/android-native/native/Cargo.toml
pnpm test
adb -s emulator-5554 install -r examples/android-native/build/outputs/apk/debug/android-native-debug.apk
```

The loader integration test remains in the Player repository. After building
and installing the current Player and example APK, run from its `src-tauri/gen/android`:

```sh
./gradlew :app:assembleArm64DebugAndroidTest -x :app:rustBuildArm64Debug
adb -s emulator-5554 install -r app/build/outputs/apk/androidTest/arm64/debug/app-arm64-debug-androidTest.apk
adb -s emulator-5554 shell am instrument -w -e class dev.verncat.player.ApkPluginsTest dev.verncat.player.test/androidx.test.runner.AndroidJUnitRunner
```

Skip rustBuild in this command only after a normal Player build; otherwise Gradle
requires a running Tauri CLI build server. The test calls the actual Kotlin loader
and Rust code from the installed APK.

[Documentation](../../docs/README.md) · [File structure](../../docs/structure.md) · [Tooling](../../docs/tooling.md)
