mod adapter;
use player_plugin_sdk::{Host, NativePlugin, Result};
use serde_json::{json, Value};

#[derive(Default)]
pub struct Plugin {
    error: String,
    adapter: adapter::Adapter,
}
impl NativePlugin for Plugin {
    fn call(&mut self, method: &str, args: Value, host: &dyn Host) -> Result {
        match method {
            "activate" => Ok(Value::Null),
            "playback" => {
                let playback = if args["playing"] == true {
                    args
                } else {
                    Value::Null
                };
                self.error = self.adapter.update(playback).err().unwrap_or_default();
                host.invalidate();
                Ok(Value::Null)
            }
            "render" => {
                let mut children = vec![
                    json!({"type":"text","text":"Your current track and artist appear in Discord. Activity is hidden while paused. Requires the Discord desktop app."}),
                ];
                if !self.error.is_empty() {
                    children.push(json!({"type":"text","tone":"error","text":self.error}));
                }
                Ok(json!({"type":"group","title":"Discord Rich Presence","children":children}))
            }
            "deactivate" => {
                self.adapter.shutdown();
                Ok(Value::Null)
            }
            _ => Err("METHOD_NOT_FOUND".into()),
        }
    }
}
#[cfg(feature = "ffi")]
player_plugin_sdk::export_plugin!(Plugin, "player.discord-rpc");

#[cfg(test)]
mod tests {
    use super::*;
    use player_plugin_sdk::testing::TestHost;
    #[test]
    fn native_ui_renders_transport_errors() {
        let h = TestHost::new(|_, _| Ok(Value::Null));
        let mut p = Plugin::default();
        p.error = "Discord disconnected".into();
        assert_eq!(
            p.call("render", json!({}), &h).unwrap()["children"][1]["text"],
            "Discord disconnected"
        );
    }
}
