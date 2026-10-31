//! Native Plugin API 2, C ABI 1. No interpreter or JavaScript runtime.
use serde_json::{json, Value};
use std::ffi::c_void;

pub use serde_json;
pub const ABI_VERSION: u32 = 1;
pub const MAX_MESSAGE_BYTES: usize = 1024 * 1024;
pub type Result<T = Value> = std::result::Result<T, String>;

pub trait Host {
    fn call(&self, method: &str, args: Value) -> Result;
    fn invalidate(&self) {
        let _ = self.call("host.invalidate", Value::Null);
    }
}
pub trait NativePlugin: Send {
    fn call(&mut self, method: &str, args: Value, host: &dyn Host) -> Result;
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Buffer {
    pub data: *mut u8,
    pub len: usize,
}
impl Buffer {
    pub fn new(bytes: Vec<u8>) -> Self {
        let bytes = bytes.into_boxed_slice();
        let len = bytes.len();
        Self {
            data: Box::into_raw(bytes) as *mut u8,
            len,
        }
    }
    /// The producing allocator must release its own buffers exactly once.
    pub unsafe extern "C" fn free(buffer: Buffer) {
        if !buffer.data.is_null() {
            drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(
                buffer.data,
                buffer.len,
            )));
        }
    }
    pub unsafe fn read(&self) -> Result<&[u8]> {
        if self.data.is_null() || self.len > MAX_MESSAGE_BYTES {
            return Err("Invalid native buffer".into());
        }
        Ok(std::slice::from_raw_parts(self.data, self.len))
    }
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct HostApi {
    pub context: *mut c_void,
    pub call: unsafe extern "C" fn(*mut c_void, *const u8, usize) -> Buffer,
    pub free: unsafe extern "C" fn(Buffer),
}
struct ForeignHost(HostApi);
impl Host for ForeignHost {
    fn call(&self, method: &str, args: Value) -> Result {
        let request =
            serde_json::to_vec(&json!({"method":method,"args":args})).map_err(|e| e.to_string())?;
        if request.len() > MAX_MESSAGE_BYTES {
            return Err("Host request exceeds 1 MiB".into());
        }
        unsafe {
            let response = (self.0.call)(self.0.context, request.as_ptr(), request.len());
            let result = response.read().and_then(|bytes| decode(bytes));
            (self.0.free)(response);
            result
        }
    }
}
pub fn encode(result: Result) -> Vec<u8> {
    let value = match result {
        Ok(value) => json!({"ok":true,"value":value}),
        Err(error) => json!({"ok":false,"error":error}),
    };
    let bytes = value.to_string().into_bytes();
    if bytes.len() > MAX_MESSAGE_BYTES {
        return br#"{"ok":false,"error":"Native response exceeds 1 MiB"}"#.to_vec();
    }
    bytes
}
pub fn decode(bytes: &[u8]) -> Result {
    if bytes.len() > MAX_MESSAGE_BYTES {
        return Err("Native response exceeds 1 MiB".into());
    }
    let result: Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    match result["ok"].as_bool() {
        Some(true) => Ok(result["value"].clone()),
        Some(false) => Err(result["error"]
            .as_str()
            .unwrap_or("Native call failed")
            .into()),
        None => Err("Invalid native response envelope".into()),
    }
}
pub fn invoke(
    plugin: &mut dyn NativePlugin,
    method: &str,
    args: Value,
    host: &dyn Host,
) -> Vec<u8> {
    encode(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            plugin.call(method, args, host)
        }))
        .unwrap_or_else(|_| Err("Native plugin panicked".into())),
    )
}

/// Export a plugin as a dynamic library. Disable the `ffi` feature when linking it into Player.
#[macro_export]
macro_rules! export_plugin {
    ($plugin:ty, $id:literal) => {
        #[no_mangle]
        pub extern "C" fn player_plugin_abi_version() -> u32 {
            $crate::ABI_VERSION
        }
        #[no_mangle]
        pub extern "C" fn player_plugin_id() -> *const std::ffi::c_char {
            concat!($id, "\0").as_ptr().cast()
        }
        #[no_mangle]
        pub extern "C" fn player_plugin_create() -> *mut std::ffi::c_void {
            std::panic::catch_unwind(|| Box::into_raw(Box::new(<$plugin>::default())).cast())
                .unwrap_or(std::ptr::null_mut())
        }
        #[no_mangle]
        pub unsafe extern "C" fn player_plugin_destroy(instance: *mut std::ffi::c_void) {
            if !instance.is_null() {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    drop(Box::from_raw(instance.cast::<$plugin>()))
                }));
            }
        }
        #[no_mangle]
        pub unsafe extern "C" fn player_plugin_call(
            instance: *mut std::ffi::c_void,
            request: *const u8,
            len: usize,
            api: $crate::HostApi,
        ) -> $crate::Buffer {
            $crate::ffi_call(instance.cast::<$plugin>(), request, len, api)
        }
        #[no_mangle]
        pub unsafe extern "C" fn player_plugin_free(buffer: $crate::Buffer) {
            $crate::Buffer::free(buffer)
        }
    };
}
pub unsafe fn ffi_call<T: NativePlugin>(
    instance: *mut T,
    request: *const u8,
    len: usize,
    api: HostApi,
) -> Buffer {
    if instance.is_null() || request.is_null() || len > MAX_MESSAGE_BYTES {
        return Buffer::new(encode(Err("Invalid native request".into())));
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result {
        let request: Value = serde_json::from_slice(std::slice::from_raw_parts(request, len))
            .map_err(|e| e.to_string())?;
        let method = request["method"].as_str().ok_or("Missing method")?;
        (*instance).call(method, request["args"].clone(), &ForeignHost(api))
    }))
    .unwrap_or_else(|_| Err("Native plugin panicked".into()));
    Buffer::new(encode(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn envelopes_reject_invalid_or_oversized_data() {
        assert_eq!(
            decode(&encode(Ok(json!({"hello":"world"})))).unwrap(),
            json!({"hello":"world"})
        );
        assert_eq!(decode(&encode(Err("failed".into()))).unwrap_err(), "failed");
        assert!(decode(b"{}").is_err());
        assert!(decode(&encode(Ok(json!("x".repeat(MAX_MESSAGE_BYTES))))).is_err());
    }
}

#[cfg(feature = "testing")]
pub mod testing {
    use super::*;
    use std::cell::RefCell;
    pub struct TestHost {
        pub settings: RefCell<Value>,
        pub calls: RefCell<Vec<(String, Value)>>,
        pub handler: Box<dyn Fn(&str, &Value) -> Result>,
    }
    impl TestHost {
        pub fn new(handler: impl Fn(&str, &Value) -> Result + 'static) -> Self {
            Self {
                settings: RefCell::new(json!({"revision":0,"values":{}})),
                calls: RefCell::new(vec![]),
                handler: Box::new(handler),
            }
        }
        pub fn count(&self, method: &str) -> usize {
            self.calls
                .borrow()
                .iter()
                .filter(|(m, _)| m == method)
                .count()
        }
    }
    impl Host for TestHost {
        fn call(&self, method: &str, args: Value) -> Result {
            self.calls.borrow_mut().push((method.into(), args.clone()));
            match method {
                "settings.get" => Ok(self.settings.borrow().clone()),
                "settings.update" => {
                    let mut settings = self.settings.borrow_mut();
                    if settings["revision"] != args["revision"] {
                        return Err("CONFLICT".into());
                    }
                    *settings = json!({"revision":settings["revision"].as_u64().unwrap()+1,"values":args["values"]});
                    Ok(settings.clone())
                }
                "host.invalidate" => Ok(Value::Null),
                _ => (self.handler)(method, &args),
            }
        }
    }
}
