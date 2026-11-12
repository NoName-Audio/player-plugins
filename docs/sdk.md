# Native SDK and C ABI

[Documentation](README.md) · [Development](development.md)

Source: [native-sdk/src/lib.rs](../native-sdk/src/lib.rs).
The current contract is Plugin API 2, with desktop C ABI 1. Rust plugins implement
`NativePlugin`; other native languages can implement the same C/JSON boundary.

## Rust interface

`NativePlugin: Send` provides `call(&mut self, method, args, host) -> Result<Value, String>`.
The host serializes calls for each activation and runs plugin code outside the UI thread.
Calls to `Host::call(method, args)` are synchronous from the plugin's perspective;
Player performs the asynchronous operation and returns its result.

| Method | Arguments / result |
| --- | --- |
| `activate` | `{pluginId, generation}`; initialize the native instance |
| `deactivate` | Release resources; host permissions have already been revoked on disable |
| `render` | `{viewId, context}` → UI node |
| `action` | `{action, payload, fields, context}` → JSON value or error |
| `search` | `{providerId, query, cursor, limit}` → `{items, nextCursor}` |
| `renderSearch` | `{ids}` → refreshed rows or `null` to keep existing rows |
| `playback` | `{title, artist, album, position, duration, playing, resource?}` |
| `event` | `{topic, payload}` from the plugin's native host services |

Positions/durations use seconds. `playback` is sent only with `playback.read`.
The native instance survives panel unmount and webview reconnect; app restart or a
new enabled generation creates a new instance. Make activation idempotent if called
outside Player's dispatcher. Native calls cannot be forcibly killed safely in-process.

## Host services

| Method | Purpose |
| --- | --- |
| `settings.get`, `settings.update` | Local per-ID settings; update with `{revision, values}` |
| `secrets.has/get/set/delete` | Credential provider access with `key`; `set` also takes `value` |
| `host.invalidate` | Refresh native UI and displayed search rows; `Host::invalidate()` wraps this |
| `host.navigate` | Open a registered own page: `{pageId, context}` |
| `host.play` | Play `{sourceToken, resource}` through Player's coordinator |
| `host.transport` | `{action: pause/resume/stop/seek, position?}` |
| `host.adopt` | Adopt an imported `{resourceId, track}` into loaded playback state |
| `host.background` | Start an asynchronous host capability call; see below |
| `http.request`, `http.download` | Network operations through the host |
| `library.import`, `library.replace` | Import a host-issued source token |
| `storage.staging` | With `storage`: own canonical staging directory `{path}` |
| `media.register` | With `media.source`: register owned `{path, resource, state}` → source token |
| `media.state` | Update transfer `{path, state}` for owned staging media |
| `media.cover` | Store an owned staging image `{path}` → cover hash |

`settings.update` replaces the settings object with the expected revision. Read the
latest revision after a conflict. Store credentials with `secrets`, not settings/logs.
Permission and generation checks apply to every host call, including callbacks from
native libraries and APKs. Navigation, playback and adoption also pass the frontend's
registered-page and capability checks.

For background work, call `host.background` with `{id, topic, method, args}`. It returns
immediately; the plugin later receives `event` with that topic and a payload
`{id, result, error?}`. Host capability checks still apply. Soulseek owns its network runtime and queues completions internally, rather than
calling a provider-specific host service.

## Native background work

Optional manifest `pollIntervalMs` (50–60000 milliseconds) requests serialized
`poll` calls while the activation is alive. Workers enqueue results using owned Rust
state; drain them during `poll` and call the scoped host there. Never retain a Host
callback on a worker thread. Disable stops scheduling and the plugin stops its workers.
Media paths must resolve inside the current plugin staging directory; symlink escapes
and other plugins’ files are rejected. Transfer states are active, completed, failed,
timed_out, or cancelled. Completed library imports use the normal source-token API.

## UI and contributions

The manifest registers `page`, `settings`, `search`, `track-action`, or `player-widget`.
Each needs `id`, `kind`, `label`; optional fields are `description`, `icon`, `order`.
Local IDs use `[a-z0-9-]`, up to 64 characters. `player-widget` requires slot
`player.actions` or `player.status`. Only `page` supports `showInMenu: false`.

Native `render` returns a tree of `group`, `text`, `input`, `button`, `toggle`, or
`results` nodes. Their fields are the same data contract across library and APK plugins:

| Node | Main fields |
| --- | --- |
| `group` | `title?`, `children?` |
| `text` | `text`, `tone?` (`error` for errors) |
| `input` | `key`, `label`, `value?`, `inputType?` (`text` / `password`) |
| `button` | `label`, `action`, `payload?`, `disabled?` |
| `toggle` | `label`, `value`, `action`, `payload?`, `disabled?` |
| `results` | `items` |

Forms submit keyed values in `fields`. Player applies its theme, shared controls,
and pending-button animation. UI data does not contain HTML, CSS, or executable code.
Limits: 12 levels, 100 children per node, 1000 result rows. Search rows are
`{id, title, subtitle?, coverHash?, status?, actions?}`. Their actions receive
`payload.resourceId`; track-menu actions receive `payload.track`.

## Desktop ABI 1

The SDK's `export_plugin!` macro exports:

| Symbol | C signature / purpose |
| --- | --- |
| `player_plugin_abi_version` | `uint32_t(void)`; returns `1` |
| `player_plugin_id` | `const char *(void)`; stable NUL-terminated ID |
| `player_plugin_create` | `void *(void)`; new activation instance |
| `player_plugin_call` | `Buffer(void *, const uint8_t *, size_t, HostApi)` |
| `player_plugin_free` | `void(Buffer)`; release a plugin-produced response |
| `player_plugin_destroy` | `void(void *)`; destroy the instance |

`Buffer` is `{uint8_t *data; size_t len;}`. `HostApi` contains `void *context`,
a host callback `Buffer(context, bytes, len)`, and `void free(Buffer)`.
The producing allocator always frees its own buffers. JSON requests are
`{method, args}`; responses are `{ok: true, value}` or `{ok: false, error}`.
Requests/responses are limited to 1 MiB. Buffers, instance pointers, and the host
callback context are valid only under their documented ownership; never retain a
host callback beyond the current call or unwind across a C/JNI boundary.
The Rust SDK catches panics at exported call boundaries.

Android uses the same JSON lifecycle/host services through Kotlin/JNI, rather than
opening desktop libraries: [APK example](../examples/android-native/README.md).

## Android reflection ABI 2

The APK entry class exposes a public no-argument constructor, `apiVersion(): Int`
(returning `2`), `pluginId(): String`, `close()`, and
`call(method: String, argsJson: String, host: (String, String) -> String): String`.
The callback's JVM type is `kotlin.jvm.functions.Function2`; its `invoke` method
is also part of the ABI. The host loads each installed APK with a parent-first
class loader, so that interface must resolve to the host's class, not a separately
renamed copy from the plugin.

Release builds shrink the host and each plugin independently. Every participating
APK must retain the callback interface and its members with:

```proguard
-keep interface kotlin.jvm.functions.Function2 { *; }
```

Plugin APKs must additionally keep their entire entry class (including JNI methods),
as shown in the example's `proguard-rules.pro`. Keeping only the entry class or only
the callback class name is insufficient: independent obfuscation can rename either
the callback type or `invoke`, breaking reflection or callback dispatch. Keep the
Kotlin runtime dependency supplied by the Kotlin Android plugin; do not relocate
the shared interface. Test a minified host against an independently minified plugin,
not only debug APKs. Existing installations built without these rules need updated
host/plugin APKs and a Player restart before reloading native libraries.

The Discord RPC plugin is desktop-only and has no Android reflection entrypoint.
