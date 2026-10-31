//! Shared JNI bridge for any NativePlugin implementation.
pub use jni;
use jni::{
    objects::{JObject, JString, JValue},
    sys::{jlong, jstring},
    JNIEnv,
};
use player_plugin_sdk::{Host, NativePlugin, Result};
use serde_json::Value;
use std::cell::RefCell;
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
pub fn invoke<'local, T: NativePlugin>(
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
                    player_plugin_sdk::invoke(&mut *(instance as *mut T), &method, args, &host)
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

pub fn create<T: NativePlugin + Default>() -> jlong {
    std::panic::catch_unwind(|| Box::into_raw(Box::new(T::default())) as jlong).unwrap_or(0)
}
pub unsafe fn close<T: NativePlugin>(instance: jlong) {
    if instance != 0 {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            drop(Box::from_raw(instance as *mut T))
        }));
    }
}
#[macro_export]
macro_rules! export_android_plugin {
    ($plugin:ty,$create:ident,$close:ident,$call:ident) => {
        #[no_mangle]
        pub extern "system" fn $create(
            _env: $crate::jni::JNIEnv,
            _this: $crate::jni::objects::JObject,
        ) -> $crate::jni::sys::jlong {
            $crate::create::<$plugin>()
        }
        #[no_mangle]
        pub unsafe extern "system" fn $close(
            _env: $crate::jni::JNIEnv,
            _this: $crate::jni::objects::JObject,
            instance: $crate::jni::sys::jlong,
        ) {
            $crate::close::<$plugin>(instance)
        }
        #[no_mangle]
        pub extern "system" fn $call<'local>(
            env: $crate::jni::JNIEnv<'local>,
            this: $crate::jni::objects::JObject,
            instance: $crate::jni::sys::jlong,
            method: $crate::jni::objects::JString<'local>,
            args: $crate::jni::objects::JString<'local>,
            callback: $crate::jni::objects::JObject<'local>,
        ) -> $crate::jni::sys::jstring {
            $crate::invoke::<$plugin>(env, this, instance, method, args, callback)
        }
    };
}
