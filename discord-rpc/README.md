# Discord RPC

Bundled package `player.discord-rpc` displays the current track and artist
in Discord Rich Presence. Supports macOS, Windows, and Linux with Discord desktop.

## Files

| File | Purpose |
| --- | --- |
| [plugin.json](plugin.json) | ID, platforms, permissions, and the `settings/general` contribution |
| [main.js](main.js) | Playback subscription, duplicate filtering, and `discord.update` calls |
| [assets/README.md](assets/README.md) | Logo attribution and PNG generation instructions |
| [assets/discord.svg](assets/discord.svg) | Vector logo source |
| [assets/discord.png](assets/discord.png) | Raster embedded as a data URL in the manifest |

## Behavior

On activation, `main.js` subscribes to playback state. It sends track changes
and significant position changes to the host API, coalesces pending updates,
and clears activity when paused. Native call errors appear in the plugin's
settings section. Replacing the PNG without updating the manifest does not
change the package image.

Permissions: `ui`, `playback.read`, `settings`, `native.discord`.
Discord IPC and reconnection are handled by
[Player's Rust adapter](https://github.com/verncat/player/blob/feat/plugins/src-tauri/src/plugins/discord.rs).

## Testing and updates

Run `pnpm test` from the player-plugins root. Worker scenarios are covered by
`pnpm test:plugins` in the Player repository; verify live activity with Discord running.
This is a bundled package with a reserved ID: packaging it does not make it eligible
for ordinary external installation. Update it through the
[Player submodule](../docs/tooling.md#player-integration).

[Documentation](../docs/README.md) · [Repository structure](../docs/structure.md)
