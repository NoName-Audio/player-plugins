import groovy.json.JsonSlurper

plugins {
    id("com.android.application") version "8.11.0"
    id("org.jetbrains.kotlin.android") version "1.9.25"
}

val pluginAbis = (findProperty("pluginAbis") as? String ?: "arm64-v8a").split(',')
val generatePluginAssets by tasks.registering(Exec::class) {
    inputs.file("plugin.json")
    inputs.dir("../../native-sdk/src")
    val manifest = JsonSlurper().parse(file("plugin.json")) as Map<*, *>
    val image = manifest["image"] as? String
    if (image != null && !image.contains(":")) inputs.file(image)
    outputs.dir(layout.buildDirectory.dir("generated/pluginAssets"))
    commandLine("cargo", "run", "--locked", "--manifest-path", "../../Cargo.toml", "-p", "player-plugin-sdk", "--bin", "player-plugin-manifest", "--", projectDir.absolutePath, layout.buildDirectory.file("generated/pluginAssets/player-plugin.json").get().asFile.absolutePath)
}
val buildPluginRust by tasks.registering(Exec::class) {
    inputs.dir("native/src")
    inputs.file("native/Cargo.toml")
    inputs.file("../../Cargo.lock")
    inputs.dir("../../native-sdk/src")
    inputs.dir("../../native-android-sdk/src")
    inputs.dir("../src")
    inputs.dir("../vendor")
    inputs.property("pluginAbis", pluginAbis)
    outputs.dir(layout.buildDirectory.dir("generated/jniLibs"))
    commandLine("cargo", "run", "--locked", "--manifest-path", "../../Cargo.toml", "-p", "player-plugin-sdk", "--bin", "player-plugin-android-build", "--", projectDir.absolutePath, pluginAbis.joinToString(","), "player_soulseek_android")
}

android {
    namespace = "dev.verncat.player.soulseek.plugin"
    compileSdk = 36
    defaultConfig {
        applicationId = "dev.verncat.player.soulseek.plugin"
        minSdk = 26
        targetSdk = 36
        versionCode = 3
        versionName = "1.0.2"
        ndk { abiFilters += pluginAbis }
    }
    buildTypes {
        getByName("release") {
            isMinifyEnabled = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
        }
    }
    sourceSets.getByName("main") {
        assets.srcDir(layout.buildDirectory.dir("generated/pluginAssets"))
        jniLibs.srcDir(layout.buildDirectory.dir("generated/jniLibs"))
    }
    // The loader searches the installed APK's nativeLibraryDir.
    packaging.jniLibs.useLegacyPackaging = true
    kotlinOptions { jvmTarget = "1.8" }
}
tasks.named("preBuild") { dependsOn(generatePluginAssets, buildPluginRust) }
