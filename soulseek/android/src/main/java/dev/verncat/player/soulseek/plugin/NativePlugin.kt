package dev.verncat.player.soulseek.plugin

import android.app.Activity
import android.os.Bundle
import org.json.JSONObject

/** Versioned reflection ABI; all argument/result data crosses as JSON. */
class NativePlugin {
    init { System.loadLibrary("player_soulseek_android") }
    private var instance = nativeCreate()
    fun apiVersion(): Int = 2
    fun pluginId(): String = "player.soulseek"
    fun call(method: String, argsJson: String, host: (String, String) -> String): String {
        check(instance != 0L) { "Native instance is closed" }
        val response = JSONObject(nativeCall(instance, method, argsJson, host))
        check(response.getBoolean("ok")) { response.optString("error", "Native call failed") }
        return response.get("value").toString()
    }
    fun close() { if (instance != 0L) { nativeClose(instance); instance = 0 } }
    private external fun nativeCreate(): Long
    private external fun nativeClose(instance: Long)
    private external fun nativeCall(instance: Long, method: String, argsJson: String, host: (String, String) -> String): String
}

class DiscoveryActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) { super.onCreate(savedInstanceState); finish() }
}
