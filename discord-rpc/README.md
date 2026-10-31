# Discord RPC

Independent desktop native package `player.discord-rpc`, for macOS, Windows and Linux.
Install its compiled `.player-plugin` from Player’s catalog, then enable it.

| File | Purpose |
| --- | --- |
| [plugin.json](plugin.json) | API 2 metadata, permissions, settings contribution |
| [src/lib.rs](src/lib.rs) | Native lifecycle, playback handling and settings UI |
| [src/adapter.rs](src/adapter.rs) | Package-owned Discord IPC worker, reconnects and throttling |
| [Cargo.toml](Cargo.toml) | Own Discord dependency and desktop C ABI exports |
| [assets/README.md](assets/README.md) | Logo attribution |

Pausing clears activity; disabling stops the worker. The public application ID can
be overridden by DISCORD_CLIENT_ID when building this package. No Discord code or
provider-specific host command is compiled into Player.
Permissions: ui, settings, playback.read.

Run `cargo test -p player-discord-plugin`. Tests cover errors, pause, coalescing,
timestamps and reconnection with a fake transport. Live Discord checks are separate.
Updates use ordinary package installation. See [delivery](../docs/tooling.md).
