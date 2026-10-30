# Creating a JavaScript plugin

[Documentation](README.md) · [File structure](structure.md) · [SDK](sdk.md)

A plugin consists of a manifest and one JavaScript file. Player runs it in a Web Worker
and renders its interface from returned data. The plugin context has no DOM, Vue,
Node.js, or direct Tauri API. Bundle dependencies into `main.js` in advance,
without external `import` / `require` statements.

## Create a directory

From the `player-plugins` root, create `my-plugin/` with the following two files.
You can also copy [examples/hello](../examples/hello/README.md) as a working starting
point, then change its ID and metadata.

`my-plugin/plugin.json`:

```json
{
  "id": "author.my-plugin",
  "name": "My plugin",
  "version": "1.0.0",
  "apiVersion": 1,
  "description": "A page inside Player.",
  "author": { "name": "Author", "url": "https://github.com/author" },
  "repository": "https://github.com/author/my-plugin",
  "platforms": ["macos", "windows", "linux", "android"],
  "permissions": ["ui"],
  "contributions": [
    { "id": "home", "kind": "page", "label": "My plugin" }
  ]
}
```

`my-plugin/main.js`:

```js
registerPlugin((ctx) => ({
  activate() {
    ctx.log('My plugin started');
  },
  render({ viewId }) {
    return { type: 'text', text: `Hello from ${viewId}` };
  }
}));
```

The factory returns its handler object synchronously. Do not make the factory
`async`; perform asynchronous initialization in `activate()`.

## plugin.json fields

| Field | Purpose and limits |
| --- | --- |
| `id` | Stable ID, up to 100 characters; dot-separated parts using `[a-z0-9-]`; `player.*` is reserved |
| `name` | Nonempty name, up to 100 UTF-8 bytes |
| `version` | `major.minor.patch`, three nonnegative integers, no prerelease |
| `apiVersion` | Currently only `1` |
| `description` | Optional description, up to 2000 UTF-8 bytes |
| `author` | Optional name string or `{ "name": "…", "url": "https://…" }` |
| `repository`, `homepage` | Optional HTTP(S) URLs without embedded credentials |
| `image` | HTTP(S) URL or PNG/JPEG/WebP/GIF data URL, up to 256 KiB of decoded data |
| `platforms` | Nonempty array of `macos`, `windows`, `linux`, `android` |
| `permissions` | Only the permissions needed, from the table below |
| `contributions` | Up to 32 contributions with unique local IDs |

Relative paths and SVG data URLs are unsupported in `image`. Use a transparent
background for a logo that should be tinted in the settings menu; cards retain
its original colors. The entire packaged JSON must fit within 4 MiB.
Player rejects unknown fields.

| Permission | Capabilities |
| --- | --- |
| `ui` | Pages, settings, search, track actions, and widgets |
| `settings` | Plugin-owned settings with revision numbers |
| `secrets` | Plugin-owned secrets through the host credential provider |
| `playback.read` | Playback state events |
| `playback.control` | Playback control |
| `network.http` | Host `http.request` / `http.download` calls |
| `library.import`, `library.replace` | Import / replacement using resources issued by the host |

`native.discord` and `native.soulseek` are reserved for bundled packages.
`native.android` is used only when connecting an installed APK: Player creates
a binding containing the package/class and file/certificate SHA-256 hashes.
Adding this permission to an ordinary JS package does not connect native code.
See the [Android example](../examples/android-native/README.md).

## Package and install

```sh
pnpm plugins:pack my-plugin /tmp/my-plugin.player-plugin
```

The result is UTF-8 JSON `{manifest, script}`, not a ZIP archive. The packer does
not run npm, bundle dependencies, embed `assets/`, or validate every Player rule.
Successful packaging alone does not guarantee that the package can be installed
or enabled.

In Player, open Settings → Plugins, select the file, review its manifest and
permissions, confirm installation, then enable the plugin. My plugin will appear
in the sidebar. Entrypoint and execution errors are checked when the Worker starts.

For URL installation, publish `plugin.json` and `main.js` in the same directory
of a public GitHub repository. Supported URLs include repository roots,
`tree/<ref>/<folder>`, and direct HTTPS links to a `.player-plugin` or `plugin.json`.
Player records the specific GitHub commit and verifies the selected package ID.

## Update

Keep the same ID, increase `version`, disable the installed plugin, and install
the new version. Settings remain in the host; local Worker state is recreated.
Update [catalog.json](catalog.md) separately to change Player's listing.
Bundled Discord/Soulseek packages update through the commit pinned by the Player build.
