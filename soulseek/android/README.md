# Soulseek Android APK

Standalone package `dev.verncat.player.soulseek.plugin`, native plugin ID player.soulseek.
It embeds the same Rust service as desktop; Player loads the installed APK through its
generic API 2 discovery/identity approval bridge.

| Path | Purpose |
| --- | --- |
| plugin.json | Android permissions, contributions and polling interval |
| build.gradle.kts | SDK/NDK build and generated assets/JNI library packaging |
| src/main/AndroidManifest.xml | Plugin discovery metadata |
| src/main/java/dev/verncat/player/soulseek/plugin/NativePlugin.kt | Kotlin entrypoint |
| native/src/lib.rs | JNI exports through native-android-sdk |
| ../src/service.rs | Shared package-owned network implementation |

From repository root: `./gradlew -p soulseek/android assembleDebug`.
Use `-PpluginAbis=arm64-v8a,armeabi-v7a,x86_64,x86` for all supported ABIs.
Output: build/outputs/apk/debug/ inside this directory. Generated files are ignored.
Install the APK, open Player → Settings → Plugins → Find APK plugins, approve and enable.
The current CI APK is debug-signed and experimental; production updates require a
persistent signing key. A changed signer can require reinstalling the APK.
