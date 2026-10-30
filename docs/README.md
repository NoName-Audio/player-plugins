# player-plugins documentation

Run commands in these documents from the `player-plugins` root unless the
**Player** repository is explicitly specified. File paths are also relative
to this repository's root.

| Task | Document |
| --- | --- |
| Locate sources and understand file responsibilities | [Repository structure](structure.md) |
| Write your first JavaScript plugin | [Development](development.md) |
| Add UI, settings, and playback events | [SDK](sdk.md) |
| Add a plugin to Player's listing | [Catalog](catalog.md) |
| Package, test, or update bundled plugins | [Tooling and delivery](tooling.md) |
| Understand the Discord integration | [discord-rpc](../discord-rpc/README.md) |
| Understand search, downloads, and track replacement | [soulseek](../soulseek/README.md) |
| Run the minimal JavaScript example | [Hello Player](../examples/hello/README.md) |
| Build an Android plugin with Rust | [Android native](../examples/android-native/README.md) |

The current contract is **Plugin API 1**. Its declarations are in [sdk.d.ts](../sdk.d.ts).
Player validates package schemas and permissions; this repository's packer only produces JSON.

Host implementation details and integration scenarios are described in
[Player's scope documents](https://github.com/verncat/player/tree/feat/plugins/docs/plugins/scope).
