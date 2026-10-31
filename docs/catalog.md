# Native plugin catalog

[Documentation](README.md) · [catalog.json](../catalog.json)

Player fetches `https://raw.githubusercontent.com/verncat/player-plugins/main/catalog.json`.
The current schema is **2**; schema 1 described script packages and is rejected.
There is no bundled fallback or disk cache. Errors clear the listing and appear as toasts.

## Format

```json
{
  "schemaVersion": 2,
  "updatedAt": "2026-10-31",
  "plugins": []
}
```

The catalog is initially empty because the previous Hello entry installed source
JavaScript. Add a native entry only after its compiled artifact URL is published.
Discord/Soulseek remain bundled packages; Android APKs are installed through Android
and connected through Find APK plugins.

Each entry requires `id`, `name`, `description`, `repository`, `addedAt`, and `platforms`.
Optional fields are `installUrl`, `updatedAt`, `version`, `author`, `image`, and `tags`.
`installUrl` should point to a built `.player-plugin`; a GitHub directory must contain
`plugin.player-plugin`. A source manifest alone is insufficient. Without installUrl,
Player uses repository. Metadata follows the [manifest rules](development.md#manifest-rules).

Dates use calendar `YYYY-MM-DD`. Preserve `addedAt` on updates; entry `updatedAt`
falls between its addition and catalog update dates. URLs use HTTPS without credentials.
Limits: 512 KiB total, 256 unique entries, 12 tags of at most 40 UTF-8 bytes each.
Unknown fields and invalid entries reject the entire response.

## Publish

1. Build native libraries for the advertised platform/CPU targets.
2. Package them with `player-plugin-pack` and publish the file, for example as a release asset.
3. Add metadata with the actual package ID, version, and installUrl; update catalog dates.
4. Run `cargo test --workspace --locked` and test installing the real package in Player.
5. Push `main`; users can select Refresh catalog.

Catalog metadata grants no permissions and does not replace the downloaded manifest.
Player verifies the selected ID and content fingerprint, then requests approval before
installation. Library code is loaded only on enable. Native plugins execute with Player's
system access; the fingerprint verifies content, not publisher identity.

Publishing the catalog does not update installed or bundled code. Bundled updates use
the [Player submodule](tooling.md#player-integration).
