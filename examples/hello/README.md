# Hello Player

A regular installable package, `example.hello`, for desktop and Android.
Use it as a starting point for a new plugin; it needs no native code or external service.

## Files

| File | Purpose |
| --- | --- |
| [plugin.json](plugin.json) | Metadata, image, permissions, and contributions |
| [main.js](main.js) | Settings loading, playback subscription, UI, and the Save action |

The manifest registers a `home` page, `preferences` settings, and a `status` widget
in `player.status`. On activation, the code reads `settings.get` and subscribes
to playback. The page and settings show the current track title and a Greeting field;
Save writes the greeting using the settings revision. The widget shows the track title
or Nothing playing. Permissions: `ui`, `settings`, `playback.read`.

## Run

From the player-plugins root:

```sh
pnpm plugins:pack examples/hello /tmp/hello.player-plugin
```

In Player: Settings → Plugins → select the file → approve permissions → enable.
Alternatively, install from
`https://github.com/verncat/player-plugins/tree/main/examples/hello`.

For your own package, copy the directory and change the manifest's `id`, `name`,
author, and links. Contributions in `plugin.json` must match the `viewId` /
`action` handlers in `main.js`. See [creating a plugin](../../docs/development.md).

[Documentation](../../docs/README.md) · [SDK](../../docs/sdk.md)
