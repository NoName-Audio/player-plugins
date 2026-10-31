# Soulseek

Independent native package `player.soulseek`, for desktop and Android.
Rust implements its UI, search, pagination, cover scheduling, preview, import,
replacement actions, and persistent transfer intentions.

| File | Purpose |
| --- | --- |
| [Cargo.toml](Cargo.toml) | Native package crate |
| [plugin.json](plugin.json) | API 2 permissions and contributions |
| [src/lib.rs](src/lib.rs) | NativePlugin implementation and behavior tests |
| [assets/README.md](assets/README.md) | Logo attribution |

| Contribution | Placement |
| --- | --- |
| `connection` | Settings: username and credential form |
| `search` | Global search provider |
| `replacement` | Internal replacement page, hidden from the sidebar |
| `find-replacement` | Track-menu action opening the internal page |
| `buffer` | Transfer status in `player.status` |

The package owns its network client, Tokio runtime, transfers and cover downloads in
[src/service.rs](src/service.rs). The modified protocol client is preserved under
[vendor/soulseek-rs-lib](vendor/soulseek-rs-lib). Player contains neither dependency.
Generic host storage/media services issue source tokens for staged downloads; normal
library import/replacement APIs save completed files. Credentials use host secrets.
Workers queue completion events; pollIntervalMs drains them on serialized native calls.

Desktop: install the compiled package from the catalog. Android: install the separate
[APK](android/README.md), then select Find APK plugins in Player. Disabling cancels
owned tasks. Uninstallation follows the normal plugin manager flow.

Run `cargo test -p player-soulseek-plugin`. Offline tests cover pagination, navigation,
preview/import intentions, status without credential prompts and remote path validation.
Live network/account behavior needs separate testing. See [SDK](../docs/sdk.md).
