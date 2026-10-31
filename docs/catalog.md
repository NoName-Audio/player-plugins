# Native plugin catalog

[Documentation](README.md) · [catalog.json](../catalog.json)

Player fetches `https://raw.githubusercontent.com/NoName-Audio/player-plugins/main/catalog.json`.
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

Official Discord and Soulseek entries are generated from successfully published
native release assets. An empty catalog is valid before the first release.

Each entry requires `id`, `name`, `description`, `repository`, `addedAt`, and `platforms`.
Optional fields are `installUrl`, `updatedAt`, `version`, `author`, `image`, `androidApkUrl`, and `tags`.
`installUrl` should point to a built `.player-plugin`; a GitHub directory must contain
`plugin.player-plugin`. A source manifest alone is insufficient. Without installUrl,
Player uses repository. Metadata follows the [manifest rules](development.md#manifest-rules).

Catalog `image` links to an image file over HTTP(S), for example
`https://raw.githubusercontent.com/NoName-Audio/player-plugins/main/soulseek/assets/soulseek.png`.
Source plugin.json uses a relative local image path. The packager embeds the file
in generated installation metadata for offline use; the release catalog generator
links to that source file at the release tag and never copies base64 into the catalog.

Dates use calendar `YYYY-MM-DD`. Preserve `addedAt` on updates; entry `updatedAt`
falls between its addition and catalog update dates. URLs use HTTPS without credentials.
Limits: 512 KiB total, 256 unique entries, 12 tags of at most 40 UTF-8 bytes each.
Unknown fields and invalid entries reject the entire response.

## Publish

You can develop and publish your plugin in your own public repository. To list it
here, fork this repository, add an entry to the `plugins` array in `catalog.json`,
and **open a pull request against main**. Set `repository` to your source repository
and `installUrl` to its published, compiled native package (for example a GitHub
Release asset). You do not need to copy your plugin's source into this repository.

1. Build and publish native packages for the platforms you advertise.
2. Add the actual ID, version, repository URL and installUrl to `catalog.json`.
3. Set entry dates and update the catalog date; preserve addedAt on later updates.
4. Test installing the package from its public URL in Player.
5. Open a pull request for maintainer review. After it is merged, users can select
   Refresh catalog to discover your plugin.

See [CONTRIBUTING.md](../CONTRIBUTING.md) for a complete example entry and submission steps.

Catalog metadata grants no permissions and does not replace the downloaded manifest.
Player verifies the selected ID and content fingerprint, then requests approval before
installation. Library code is loaded only on enable. Native plugins execute with Player's
system access; the fingerprint verifies content, not publisher identity.

Publishing the catalog does not update installed code. On Android, `androidApkUrl`
is an HTTPS APK download link and requires the android platform tag. Download APK
opens it externally; install with Android, then use Find APK plugins to approve the
installed APK identity. See the [release pipeline](tooling.md#release-pipeline).
