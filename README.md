# Player plugins

Plugin sources for [Player](https://github.com/verncat/player): bundled Discord RPC
and Soulseek packages, an installable JavaScript example, an Android APK example,
the SDK, and the public catalog.

## Repository contents

| Path | Purpose |
| --- | --- |
| [discord-rpc/](discord-rpc/README.md) | Discord Rich Presence package for desktop |
| [soulseek/](soulseek/README.md) | Music search, preview, and download package |
| [examples/hello/](examples/hello/README.md) | Example of a regular installable plugin |
| [examples/android-native/](examples/android-native/README.md) | Standalone Android APK with Kotlin and Rust JNI |
| [sdk.d.ts](sdk.d.ts) | Plugin API declarations, version 1 |
| [catalog.json](catalog.json) | Plugin listing for Settings → Plugins |
| [scripts/](docs/tooling.md) | JavaScript packaging and Rust builds for Android |
| [docs/](docs/README.md) | Author and maintainer guides |

See the [repository structure](docs/structure.md) for the full file tree and responsibilities.

## Get started

Requires Node.js 20+ and pnpm. Packaging a JavaScript plugin does not require
Player's source code. Run these commands from the **player-plugins** repository root.

```sh
git clone https://github.com/verncat/player-plugins.git
cd player-plugins
pnpm install
pnpm test
pnpm test:plugin-sdk
pnpm plugins:pack examples/hello /tmp/hello.player-plugin
```

Install the file through Settings → Plugins, or use this URL:
`https://github.com/verncat/player-plugins/tree/main/examples/hello`.

## Documentation

- [Create a plugin](docs/development.md): directory layout, manifest, code, packaging, and installation.
- [SDK and UI](docs/sdk.md): lifecycle, API calls, and interface elements.
- [Publish to the catalog](docs/catalog.md): JSON format and adding an entry.
- [Tooling and Player integration](docs/tooling.md): commands, checks, the submodule, and updates.

Player fetches the catalog from
`https://raw.githubusercontent.com/verncat/player-plugins/main/catalog.json`.
Discord and Soulseek ship with Player from a pinned commit of this repository;
the runtime and Rust adapters live in the Player repository.
