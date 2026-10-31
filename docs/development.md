# Creating a native plugin

[Documentation](README.md) · [SDK](sdk.md) · [File structure](structure.md)

## Create a Rust package

Copy [examples/hello](../examples/hello/README.md), add the new crate to the root
workspace's `members`, and change its Cargo package name and plugin ID.
The ID in `plugin.json` and `export_plugin!` must match.

A desktop crate builds both `rlib` and `cdylib` and enables FFI exports by default:

```toml
[lib]
crate-type = ["rlib", "cdylib"]

[features]
default = ["ffi"]
ffi = []

[dependencies]
player-plugin-sdk.workspace = true
serde_json.workspace = true
```

`plugin.json` declares API 2, its platforms, host permissions, and UI contributions.
For example, a page needs the `ui` permission and a `page` contribution.

```json
{
  "id": "author.my-plugin",
  "name": "My native plugin",
  "version": "1.0.0",
  "apiVersion": 2,
  "description": "A native page inside Player.",
  "author": {"name": "Author", "url": "https://github.com/author"},
  "repository": "https://github.com/author/my-plugin",
  "platforms": ["macos", "windows", "linux"],
  "permissions": ["ui"],
  "contributions": [{"id": "home", "kind": "page", "label": "My plugin"}]
}
```

Implement the shared Rust trait in `src/lib.rs`:

```rust
use player_plugin_sdk::{Host, NativePlugin, Result};
use serde_json::{json, Value};

#[derive(Default)]
pub struct Plugin;

impl NativePlugin for Plugin {
    fn call(&mut self, method: &str, _args: Value, _host: &dyn Host) -> Result {
        match method {
            "activate" | "deactivate" => Ok(Value::Null),
            "render" => Ok(json!({"type": "text", "text": "Hello from Rust"})),
            _ => Err("METHOD_NOT_FOUND".into()),
        }
    }
}

#[cfg(feature = "ffi")]
player_plugin_sdk::export_plugin!(Plugin, "author.my-plugin");
```

All UI/action/search/event logic belongs to native code. The frontend only renders
returned data and forwards approved requests. No executable script is packaged.

## Manifest rules

| Field | Rule |
| --- | --- |
| `id` | Up to 100 ASCII characters, dot-separated `[a-z0-9-]` parts; official IDs are installable too |
| `name` | Nonempty, up to 100 UTF-8 bytes |
| `version` | Three nonnegative integers: `major.minor.patch` |
| `apiVersion` | Exactly `2` |
| `description` | Optional, up to 2000 UTF-8 bytes |
| `author` | Optional name string or `{name, url?}` |
| `repository`, `homepage` | Optional HTTP(S) URLs without credentials |
| `image` | HTTP(S) URL or PNG/JPEG/WebP/GIF base64 data URL, at most 256 KiB decoded |
| `platforms` | Nonempty array of `macos`, `windows`, `linux`, `android` |
| `permissions` | Host capabilities needed by the package |
| `contributions` | At most 32 entries with unique local IDs |

Unknown fields are rejected. For settings-menu logo tinting, use a transparent image.
Cards retain the image's original colors.

Host permissions are `ui`, `settings`, `secrets`, `playback.read`, `playback.control`,
`network.http`, `storage`, `media.source`, `library.import`, and `library.replace`.
Provider-specific native permissions are not supported. `native.android` requires an
installed APK binding created by Player, not a manually supplied source manifest value.
Native code has Player's OS access; these capabilities protect host API routing,
not the process from the native plugin.

## Build and package

Build the example for your current Rust target:

```sh
cargo build -p player-hello-plugin
```

| Package target key | Typical native library |
| --- | --- |
| `macos-aarch64`, `macos-x86_64` | `target/debug/libplayer_hello_plugin.dylib` |
| `linux-aarch64`, `linux-x86_64` | `target/debug/libplayer_hello_plugin.so` |
| `windows-aarch64`, `windows-x86_64` | `target/debug/player_hello_plugin.dll` |

The target key must match the binary's actual CPU/OS. Cross-compiled Cargo outputs
also include the Rust target triple in their directory. Package one or more libraries:

```sh
cargo run -p player-plugin-sdk --bin player-plugin-pack -- examples/hello /tmp/hello.player-plugin macos-aarch64=target/debug/libplayer_hello_plugin.dylib
```

Additional `os-arch=library-path` arguments add other targets. The packer embeds
base64 bytes and SHA-256 for each target. Maximums: 64 MiB per library, 96 MiB per
package, and eight artifacts. Player checks the package/fingerprint before approval
and loads only the matching target after the plugin is enabled.

Install the file through Settings → Plugins, review its native-code access, and enable.
For URL installation, publish a built `.player-plugin`; a GitHub directory URL must
contain `plugin.player-plugin`. Source-only `plugin.json` links are not installable.

On hardened macOS builds, the library must meet the app's existing code-signing
validation requirements. The loader does not disable macOS library validation.

## Updates and migration

Keep the ID, increase the version, disable the plugin, then install the new package.
Settings survive updates. Each activation creates a fresh instance; callbacks from
old generations are rejected. Libraries remain resident until app exit so disabled
code is not unloaded from underneath native threads. Native implementations must
stop their workers and release resources in `deactivate` / instance destruction.
Android APK updates require a new scan/approval and a process restart if the old
JNI library was loaded.

API 1 script packages are rejected. Migration removes their installed executable
packages while preserving settings records and credentials; those records are disabled
so a native replacement can be installed under the same ID. Former built-in package IDs retain
their settings and credentials, but require installing a standalone package. [Catalog publication](catalog.md) is separate from package installation.
