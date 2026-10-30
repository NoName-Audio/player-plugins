# Soulseek

Bundled package `player.soulseek` provides music search, preview, library imports,
and track replacement. Platforms: macOS, Windows, Linux, and Android.

## Files

| File | Purpose |
| --- | --- |
| [plugin.json](plugin.json) | Permissions, settings, search, track action, internal page, and player widget |
| [main.js](main.js) | Connection UI, search, result cursors, covers, and transfer intentions |
| [assets/README.md](assets/README.md) | Logo attribution |
| [assets/soulseek.png](assets/soulseek.png) | Source PNG embedded as a data URL in the manifest |

## Player contributions

| Contribution | Purpose |
| --- | --- |
| `connection` / `settings` | Username and password |
| `search` / `search` | Global search provider |
| `replacement` / `page` | Internal replacement search page, hidden from the sidebar |
| `find-replacement` / `track-action` | Find another version in a track's context menu |
| `buffer` / `player-widget` | Soulseek state in `player.status` |

`main.js` calls the `soulseek.*` API, follows transfer events, and delegates playback
to Player's main coordinator. Preview/save intentions are stored in settings with
an activation generation so they can be restored after a Worker reload.
Passwords use `secrets`, not ordinary settings JSON.

Soulseek network connections and transfers are implemented in
[Player's Rust adapter](https://github.com/verncat/player/blob/feat/plugins/src-tauri/src/plugins/soulseek.rs).
The package requests `ui`, `settings`, `secrets`, `playback.read`, `playback.control`,
`native.soulseek`, `library.import`, and `library.replace`.

## Testing and updates

Run `pnpm test` from the player-plugins root. In the Player repository,
`pnpm test:plugins` covers search, intention recovery, preview/import, and track-menu
navigation. A live connection requires a separate check with a Soulseek account.
This bundled package updates through the [Player submodule](../docs/tooling.md#player-integration),
not by installing an external `.player-plugin` with the same reserved ID.

[Documentation](../docs/README.md) · [SDK](../docs/sdk.md)
