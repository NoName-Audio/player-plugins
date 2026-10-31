player_plugin_android_sdk::export_android_plugin!(
    player_soulseek_plugin::Plugin,
    Java_dev_verncat_player_soulseek_plugin_NativePlugin_nativeCreate,
    Java_dev_verncat_player_soulseek_plugin_NativePlugin_nativeClose,
    Java_dev_verncat_player_soulseek_plugin_NativePlugin_nativeCall
);
