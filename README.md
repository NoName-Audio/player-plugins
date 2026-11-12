# Player native plugins

Native extensions for [Player](https://github.com/NoName-Audio/player). All plugin logic,
including UI descriptions and actions, runs in compiled native code. There is no
JavaScript plugin runtime, `main.js`, or script SDK.

| Path | Purpose |
| --- | --- |
| [native-sdk/](docs/sdk.md) | Rust trait, C ABI 1, packaging and Android build tools |
| [discord-rpc/](discord-rpc/README.md) | Independent native Discord package |
| [soulseek/](soulseek/README.md) | Independent native search, preview, and import package |
| [examples/hello/](examples/hello/README.md) | Installable desktop native library example |
| [examples/android-native/](examples/android-native/README.md) | Standalone Kotlin/Rust APK example |
| [catalog.json](catalog.json) | Public native package listing, schema 2 |
| [docs/](docs/README.md) | Author and maintainer documentation |
| [Attachment targets](docs/attachment-targets.md) | Generated Player target and context reference |

Full layout: [repository structure](docs/structure.md).

## Get started

Requires a current stable Rust toolchain. Run from this repository's root:

```sh
cargo test --workspace --locked
cargo build -p player-hello-plugin
```

Package Hello on macOS arm64:

```sh
cargo run -p player-plugin-sdk --bin player-plugin-pack -- examples/hello /tmp/hello.player-plugin macos-aarch64=target/debug/libplayer_hello_plugin.dylib
```

Use the matching library and target key for your platform; see [development](docs/development.md).
Install the resulting file in Player's Settings → Plugins, review it, then enable it.
Native extensions execute inside Player with its system access. Permission declarations
control the host API, not an OS sandbox.

## Platforms

Desktop extensions load as `.dylib`, `.so`, or `.dll` through a versioned C ABI.
Android extensions are separately installed APKs with Kotlin entrypoints and Rust JNI.
Both receive lifecycle, render, action, search, playback, and event requests.
Player renders returned UI data using its shared components and theme.
Optional API 2 [component attachments](docs/sdk.md#component-attachments-optional-api-2-extension)
render native UI before/after explicitly exposed targets such as `AlbumCard`,
with per-instance context and actions. There is no API/ABI bump; older strict
API 2 hosts reject manifests with `attachments`, so these packages need an updated host.

Use the [attachment target reference](docs/attachment-targets.md) to select a
supported target and inspect its context. This reference is generated from
Player's explicit `withPluginAttachments` registrations (TypeScript AST) and
their source context types, rather than maintained as a separate target list.
In the [Player repository](https://github.com/NoName-Audio/player), run
`pnpm attachments:docs` to write `plugins/docs/attachment-targets.md`, or
`pnpm attachments:docs --output /tmp/attachment-targets.md` to select another
output path. Application builds never generate this file. Player CI provides a
generated preview artifact; a published Player release proposes an update here
through a cross-repository pull request, not a direct default-branch push.


Discord and Soulseek are separately installed native packages. Their IPC/network
implementations and dependencies are built here, never linked into Player. Player
uses only the generic SDK from its pinned submodule. Install or update plugins
without rebuilding Player.
On hardened macOS builds, libraries must satisfy the app's existing signature validation;
this change does not disable library validation.

The catalog endpoint is
`https://raw.githubusercontent.com/NoName-Audio/player-plugins/main/catalog.json`.
Legacy script packages are not listed. Publish compiled native artifacts before adding
entries; see [catalog publishing](docs/catalog.md).

## Add your plugin to the catalog

Publish your plugin in your own repository, then **open a pull request** here adding
an entry to [catalog.json](catalog.json). Set `repository` to your repository and
`installUrl` to your published native package. You do not need to copy the plugin's
source code into this repository. See [submission instructions and an example entry](CONTRIBUTING.md).
