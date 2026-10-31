use player_plugin_sdk::{Host, NativePlugin, Result};
use serde_json::{json, Value};

#[derive(Default)]
pub struct Plugin {
    settings: Value,
    title: String,
}
impl NativePlugin for Plugin {
    fn call(&mut self, method: &str, args: Value, host: &dyn Host) -> Result {
        match method {
            "activate" => {
                self.settings = host.call("settings.get", json!({}))?;
                self.title = "Nothing playing".into();
                Ok(Value::Null)
            }
            "playback" => {
                self.title = if args["playing"] == true {
                    args["title"].as_str().unwrap_or("").into()
                } else {
                    "Nothing playing".into()
                };
                host.invalidate();
                Ok(Value::Null)
            }
            "render" => {
                if args["viewId"] == "status" {
                    return Ok(json!({"type":"text","text":self.title}));
                }
                Ok(
                    json!({"type":"group","title":self.settings["values"]["greeting"].as_str().unwrap_or("Hello Player"),"children":[
                    {"type":"text","text":format!("Now playing: {}",self.title)},
                    {"type":"input","key":"greeting","label":"Greeting","value":self.settings["values"]["greeting"].as_str().unwrap_or("")},
                    {"type":"button","label":"Save","action":"save"}]}),
                )
            }
            "action" if args["action"] == "save" => {
                let latest = host.call("settings.get", json!({}))?;
                let mut values = latest["values"].clone();
                values["greeting"] = json!(args["fields"]["greeting"]
                    .as_str()
                    .unwrap_or("")
                    .chars()
                    .take(100)
                    .collect::<String>());
                self.settings = host.call(
                    "settings.update",
                    json!({"revision":latest["revision"],"values":values}),
                )?;
                host.invalidate();
                Ok(Value::Null)
            }
            "deactivate" => Ok(Value::Null),
            _ => Err("METHOD_NOT_FOUND".into()),
        }
    }
}
#[cfg(feature = "ffi")]
player_plugin_sdk::export_plugin!(Plugin, "example.hello");

#[cfg(test)]
mod tests {
    use super::*;
    use player_plugin_sdk::testing::TestHost;
    #[test]
    fn settings_and_playback_drive_native_ui() {
        let host = TestHost::new(|_, _| Ok(Value::Null));
        let mut plugin = Plugin::default();
        plugin.call("activate", json!({}), &host).unwrap();
        plugin
            .call(
                "action",
                json!({"action":"save","fields":{"greeting":"Native hello"}}),
                &host,
            )
            .unwrap();
        assert_eq!(
            plugin
                .call("render", json!({"viewId":"home"}), &host)
                .unwrap()["title"],
            "Native hello"
        );
        plugin
            .call("playback", json!({"playing":true,"title":"A track"}), &host)
            .unwrap();
        assert_eq!(
            plugin
                .call("render", json!({"viewId":"status"}), &host)
                .unwrap()["text"],
            "A track"
        );
        plugin
            .call("playback", json!({"playing":false}), &host)
            .unwrap();
        assert_eq!(
            plugin
                .call("render", json!({"viewId":"status"}), &host)
                .unwrap()["text"],
            "Nothing playing"
        );
    }
}
