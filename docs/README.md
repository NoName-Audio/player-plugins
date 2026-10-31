# Native plugin documentation

Commands and paths refer to the `player-plugins` root unless **Player** is explicitly named.

| Task | Guide |
| --- | --- |
| Locate source files and build outputs | [Repository structure](structure.md) |
| Create and package a desktop native extension | [Development](development.md) |
| Implement lifecycle, UI, events, and host calls | [SDK and ABI](sdk.md) |
| Publish a compiled native package | [Catalog](catalog.md) |
| Run tools, checks, and update Player's submodule | [Tooling](tooling.md) |
| Understand the independent integrations | [Discord](../discord-rpc/README.md), [Soulseek](../soulseek/README.md) |
| Run a native library example | [Hello](../examples/hello/README.md) |
| Build an Android APK with Rust | [Android native](../examples/android-native/README.md) |

Current contracts: **Plugin API 2**, **desktop C ABI 1**, **catalog schema 2**.
All executable plugin code is native. JSON describes manifests, calls, and UI;
it is not an executable scripting language.
