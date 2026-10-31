# Hello native plugin

`example.hello` is an installable desktop native library. Rust implements the page,
settings, Save action, playback events, and footer widget. There is no script component.

| File | Purpose |
| --- | --- |
| [Cargo.toml](Cargo.toml) | Rust crate producing a shared library |
| [plugin.json](plugin.json) | API 2 manifest and contributions |
| [src/lib.rs](src/lib.rs) | NativePlugin implementation and C ABI exports |

From the repository root, build and test:

```sh
cargo test -p player-hello-plugin
cargo build -p player-hello-plugin
```

Package on macOS arm64:

```sh
cargo run -p player-plugin-sdk --bin player-plugin-pack -- examples/hello /tmp/hello.player-plugin macos-aarch64=target/debug/libplayer_hello_plugin.dylib
```

Use the matching target and library on Windows/Linux; see [development](../../docs/development.md).
Install the `.player-plugin` file through Settings → Plugins, approve its native-code
access, and enable. A source-only repository URL is not installable until it contains
a built `plugin.player-plugin` or links to a published native release package.

For a new plugin, copy this crate, add it to the workspace, rename its Cargo package,
and change the manifest and `export_plugin!` IDs together. Preserve a plugin's ID
when updating an installed version.

[Documentation](../../docs/README.md) · [SDK and ABI](../../docs/sdk.md)
