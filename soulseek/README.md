# Soulseek

Bundled native package `player.soulseek`, for desktop and Android.
Rust implements its UI, search, pagination, cover scheduling, preview, import,
replacement actions, and persistent transfer intentions.

| File | Purpose |
| --- | --- |
| [Cargo.toml](Cargo.toml) | Native package crate |
| [plugin.json](plugin.json) | API 2 permissions and contributions |
| [src/lib.rs](src/lib.rs) | NativePlugin implementation and behavior tests |
| [assets/README.md](assets/README.md) | Logo attribution |

| Contribution | Placement |
| --- | --- |
| `connection` | Settings: username and credential form |
| `search` | Global search provider |
| `replacement` | Internal replacement page, hidden from the sidebar |
| `find-replacement` | Track-menu action opening the internal page |
| `buffer` | Transfer status in `player.status` |

The plugin uses Player's Rust `soulseek.*` services for network operations. Preview
plays a host-issued source token once buffered; completed transfers import or replace
tracks and adopt the result in Player's playback state. Generation-scoped intentions
persist in settings so the native instance can recover after a UI reconnect.
Passwords use the host credential provider. Background cover calls do not block render.

Host service source:
[soulseek.rs](https://github.com/verncat/player/blob/feat/plugins/src-tauri/src/plugins/soulseek.rs).
Permissions: `ui`, `settings`, `secrets`, `playback.read`, `playback.control`,
`native.soulseek`, `library.import`, `library.replace`.

Run `cargo test -p player-soulseek-plugin`. Tests cover paging without dropped matches,
track-menu navigation, buffered preview, import after instance recovery, failed
transfers, and stale intentions. Live network checks remain separate.
Update this bundled package through the [Player submodule](../docs/tooling.md#player-integration).

[Documentation](../docs/README.md) · [Native SDK](../docs/sdk.md)
