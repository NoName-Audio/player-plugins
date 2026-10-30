# Tooling, checks, and delivery

[Documentation](README.md) · [File structure](structure.md)

## player-plugins commands

Run all commands in this table from this repository's root.

| Command | Purpose |
| --- | --- |
| `pnpm install --frozen-lockfile` | Install pinned tooling dependencies |
| `pnpm test` | Package all four plugins and check JS syntax and catalog consistency |
| `pnpm test:plugin-sdk` | Check `sdk.d.ts` declarations, not every plugin's `main.js` |
| `pnpm plugins:pack <folder> [output]` | Produce a JSON package from two files |
| `pnpm plugins:android:build` | Build the Android example's debug APK through the Gradle wrapper |

[package-plugin.mjs](../scripts/package-plugin.mjs) reads `plugin.json` and `main.js`,
produces `{manifest, script}`, and checks the 4 MiB limit. Without an output path,
it writes `<folder>/<id>-<version>.player-plugin`. This packages files; it does not
compile code or fully validate Player's schema and permission rules.

Gradle calls [build-android-plugin.mjs](../scripts/build-android-plugin.mjs) with
the example directory and comma-separated ABIs. The script selects the NDK from
`ANDROID_NDK_HOME` or the SDK from `ANDROID_HOME` / `ANDROID_SDK_ROOT`, runs Cargo
with `--locked --release`, and copies `.so` files to `build/generated/jniLibs/<abi>/`.
Gradle handles APK and asset packaging. See the [Android example](../examples/android-native/README.md).

## Automated checks

[check.yml](../.github/workflows/check.yml) runs on push and pull request:
Node.js 22, pnpm 10, installation from the lockfile, `pnpm test`, and `pnpm test:plugin-sdk`.
It does not build APKs or test live Discord/Soulseek connections.

[packages.test.mjs](../tests/packages.test.mjs) uses actual package sources.
It checks that the packer works from another working directory, IDs are unique,
scripts parse successfully, and catalog install URLs point to existing local
packages with matching IDs, names, versions, and platforms.

This repository's tests do not run the Worker or native gateway. Integration checks
run in the **Player** repository, for example:

```sh
pnpm test:plugins
cargo test --manifest-path src-tauri/Cargo.toml --lib
```

## Player integration

In Player, `plugins/` is a git submodule pinned to a specific commit of this repository.
A sibling `../player-plugins` checkout and `Player/plugins/` are independent:
editing files in the former does not automatically update the latter.

Before building Player for the first time, run this from the **Player** root:

```sh
git submodule update --init -- plugins
```

To include updated bundled packages in a build, publish their changes to this
repository first, then run these commands from the **Player** root:

```sh
git -C plugins fetch origin main
git -C plugins checkout --detach origin/main
git add plugins
git commit -m "Update bundled plugins"
```

Test Player and publish its commit through the usual workflow. Its release workflow
fetches the pinned submodule; changes to this repository's `main` do not replace
code in an already built app. Changes only to `catalog.json` become available
without updating the gitlink or rebuilding Player.

The runtime, UI renderer, and native adapters remain in Player. Packages
`player.discord-rpc` and `player.soulseek` cannot be reinstalled as ordinary
external `.player-plugin` files.
