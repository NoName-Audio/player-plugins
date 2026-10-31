# Discord RPC

Bundled native package `player.discord-rpc`, for macOS, Windows, and Linux.
All lifecycle, playback handling, and UI logic is implemented in Rust.

| File | Purpose |
| --- | --- |
| [Cargo.toml](Cargo.toml) | Native `rlib` / `cdylib`; FFI exports disabled when linked into Player |
| [plugin.json](plugin.json) | API 2 metadata, permissions, and settings contribution |
| [src/lib.rs](src/lib.rs) | NativePlugin implementation, UI, presence updates, tests |
| [assets/README.md](assets/README.md) | Logo attribution and PNG generation |

The native package receives `playback`, calls `discord.update`, and renders its
settings tree. Pausing clears activity. Errors appear in the native-rendered UI.
Player's existing Rust adapter handles IPC, throttling, and reconnection:
[discord.rs](https://github.com/verncat/player/blob/feat/plugins/src-tauri/src/plugins/discord.rs).

Permissions: `ui`, `settings`, `playback.read`, `native.discord`.
Run `cargo test -p player-discord-plugin`. Host adapter tests and live Discord checks
run in Player. The reserved `player.*` ID is not externally installable; update the
[Player submodule](../docs/tooling.md#player-integration) to ship a new bundled version.

[Documentation](../docs/README.md) · [Native SDK](../docs/sdk.md)
