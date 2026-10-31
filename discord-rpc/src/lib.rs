use player_plugin_sdk::{Host, NativePlugin, Result};
use serde_json::{json, Value};

#[derive(Default)]
pub struct Plugin {
    error: String,
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
                self.error = host
                    .call("discord.update", json!({"playback":playback}))
                    .err()
                    .unwrap_or_default();
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
            "deactivate" => host.call("discord.update", json!({"playback":null})),
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
    fn pause_clears_presence_and_errors_are_rendered() {
        let host = TestHost::new(|_, _| Ok(Value::Null));
        let mut plugin = Plugin::default();
        plugin
            .call("playback", json!({"playing":true,"title":"Song"}), &host)
            .unwrap();
        plugin
            .call("playback", json!({"playing":false,"title":"Song"}), &host)
            .unwrap();
        assert!(host
            .calls
            .borrow()
            .iter()
            .filter(|(m, _)| m == "discord.update")
            .last()
            .unwrap()
            .1["playback"]
            .is_null());
        let failed = TestHost::new(|_, _| Err("Discord disconnected".into()));
        plugin
            .call("playback", json!({"playing":true}), &failed)
            .unwrap();
        assert_eq!(
            plugin.call("render", json!({}), &failed).unwrap()["children"][1]["text"],
            "Discord disconnected"
        );
    }
}
