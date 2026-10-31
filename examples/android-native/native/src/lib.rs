use player_plugin_sdk::{Host, NativePlugin, Result};
use serde_json::{json, Value};

#[derive(Default)]
struct Plugin {
    message: String,
    count: u64,
}
impl NativePlugin for Plugin {
    fn call(&mut self, method: &str, args: Value, host: &dyn Host) -> Result {
        match method {
            "activate" => {
                let settings = host.call("settings.get", json!({}))?;
                self.message = settings["values"]["lastMessage"]
                    .as_str()
                    .unwrap_or("Loaded from an APK")
                    .into();
                Ok(Value::Null)
            }
            "render" => Ok(
                json!({"type":"group","title":"Android native plugin","children":[
                {"type":"text","text":format!("{} · Rust via JNI · {}",self.message,std::env::consts::ARCH)},
                {"type":"text","text":format!("Rust calls: {}",self.count)},
                {"type":"input","key":"message","label":"Message","value":"Hello from Player"},
                {"type":"button","action":"ping","label":"Call Rust"}]}),
            ),
            "action" if args["action"] == "ping" => {
                self.message = args["fields"]["message"]
                    .as_str()
                    .unwrap_or("Hello from Player")
                    .into();
                self.count += 1;
                let settings = host.call("settings.get", json!({}))?;
                let mut values = settings["values"].clone();
                values["lastMessage"] = json!(self.message);
                host.call(
                    "settings.update",
                    json!({"revision":settings["revision"],"values":values}),
                )?;
                host.invalidate();
                Ok(Value::Null)
            }
            "deactivate" => Ok(Value::Null),
            _ => Err("METHOD_NOT_FOUND".into()),
        }
    }
}
player_plugin_android_sdk::export_android_plugin!(
    Plugin,
    Java_dev_verncat_player_example_nativeplugin_NativePlugin_nativeCreate,
    Java_dev_verncat_player_example_nativeplugin_NativePlugin_nativeClose,
    Java_dev_verncat_player_example_nativeplugin_NativePlugin_nativeCall
);
#[cfg(test)]
mod tests {
    use super::*;
    use player_plugin_sdk::testing::TestHost;
    #[test]
    fn ui_and_actions_are_native_and_use_host_settings() {
        let h = TestHost::new(|_, _| Ok(Value::Null));
        let mut p = Plugin::default();
        p.call("activate", json!({}), &h).unwrap();
        p.call(
            "action",
            json!({"action":"ping","fields":{"message":"Native Rust"}}),
            &h,
        )
        .unwrap();
        let ui = p.call("render", json!({}), &h).unwrap();
        assert!(ui["children"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Native Rust · Rust via JNI"));
        assert_eq!(h.settings.borrow()["values"]["lastMessage"], "Native Rust");
        assert_eq!(p.count, 1);
    }
}
