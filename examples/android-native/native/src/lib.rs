use jni::{
    objects::{JObject, JString, JValue},
    sys::{jlong, jstring},
    JNIEnv,
};
use player_plugin_sdk::{Host, NativePlugin, Result};
use serde_json::{json, Value};
use std::cell::RefCell;

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
struct AndroidHost<'a, 'local> {
    env: RefCell<&'a mut JNIEnv<'local>>,
    callback: JObject<'local>,
}
impl Host for AndroidHost<'_, '_> {
    fn call(&self, method: &str, args: Value) -> Result {
        let mut env = self.env.borrow_mut();
        let method = env.new_string(method).map_err(|e| e.to_string())?;
        let args = env
            .new_string(args.to_string())
            .map_err(|e| e.to_string())?;
        let response = env
            .call_method(
                &self.callback,
                "invoke",
                "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
                &[
                    JValue::Object(method.as_ref()),
                    JValue::Object(args.as_ref()),
                ],
            )
            .and_then(|v| v.l())
            .map_err(|e| e.to_string())?;
        let response = JString::from(response);
        let text: String = env.get_string(&response).map_err(|e| e.to_string())?.into();
        player_plugin_sdk::decode(text.as_bytes())
    }
}
#[no_mangle]
pub extern "system" fn Java_dev_verncat_player_example_nativeplugin_NativePlugin_nativeCreate(
    _env: JNIEnv,
    _this: JObject,
) -> jlong {
    Box::into_raw(Box::new(Plugin::default())) as jlong
}
#[no_mangle]
pub unsafe extern "system" fn Java_dev_verncat_player_example_nativeplugin_NativePlugin_nativeClose(
    _env: JNIEnv,
    _this: JObject,
    instance: jlong,
) {
    if instance != 0 {
        drop(Box::from_raw(instance as *mut Plugin));
    }
}
#[no_mangle]
pub extern "system" fn Java_dev_verncat_player_example_nativeplugin_NativePlugin_nativeCall<
    'local,
>(
    mut env: JNIEnv<'local>,
    _this: JObject,
    instance: jlong,
    method: JString<'local>,
    args: JString<'local>,
    callback: JObject<'local>,
) -> jstring {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Vec<u8> {
        let parse = (|| -> Result<(String, Value)> {
            if instance == 0 {
                return Err("Native instance is closed".into());
            }
            let method: String = env.get_string(&method).map_err(|e| e.to_string())?.into();
            let args: String = env.get_string(&args).map_err(|e| e.to_string())?.into();
            if args.len() > player_plugin_sdk::MAX_MESSAGE_BYTES {
                return Err("Native request exceeds 1 MiB".into());
            }
            Ok((
                method,
                serde_json::from_str(&args).map_err(|e| e.to_string())?,
            ))
        })();
        match parse {
            Ok((method, args)) => {
                let host = AndroidHost {
                    env: RefCell::new(&mut env),
                    callback,
                };
                unsafe {
                    player_plugin_sdk::invoke(&mut *(instance as *mut Plugin), &method, args, &host)
                }
            }
            Err(e) => player_plugin_sdk::encode(Err(e)),
        }
    }))
    .unwrap_or_else(|_| player_plugin_sdk::encode(Err("Native panic".into())));
    env.new_string(String::from_utf8_lossy(&result))
        .map(|value| value.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

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
