# Plugin catalog

[Documentation](README.md) · [catalog.json](../catalog.json)

Player fetches the listing from
`https://raw.githubusercontent.com/verncat/player-plugins/main/catalog.json`.
The repository must be public. Fetching the catalog does not download executable
code: the package is downloaded separately after selecting Install.

## Entry format

```json
{
  "schemaVersion": 1,
  "updatedAt": "2026-10-31",
  "plugins": [
    {
      "id": "example.hello",
      "name": "Hello Player",
      "description": "An example extension with a page, settings and playback events.",
      "repository": "https://github.com/verncat/player-plugins",
      "installUrl": "https://github.com/verncat/player-plugins/tree/main/examples/hello",
      "addedAt": "2026-10-27",
      "updatedAt": "2026-10-31",
      "version": "1.0.0",
      "author": "Player contributors",
      "platforms": ["macos", "windows", "linux", "android"],
      "tags": ["example", "playback", "settings"]
    }
  ]
}
```

| Field | Rule |
| --- | --- |
| `schemaVersion` | Whole-file schema version; currently `1` |
| Catalog `updatedAt` | Calendar date in `YYYY-MM-DD` format |
| `plugins` | At most 256 entries with unique IDs |
| `id`, `name`, `description`, `repository`, `addedAt`, `platforms` | Required entry fields |
| `installUrl` | HTTPS link to a `plugin.json` + `main.js` directory or a packaged `.player-plugin`; defaults to `repository` |
| `version`, `author`, `image`, `updatedAt`, `tags` | Optional entry metadata |
| `tags` | Up to 12 nonempty tags, each up to 40 UTF-8 bytes |

IDs, names, platforms, authors, images, and versions follow the
[manifest rules](development.md#pluginjson-fields). Repository/install URLs must
use HTTPS without embedded credentials. `addedAt` cannot be later than the catalog
date; an entry's update date must fall between its addition date and the catalog date.
The entire response is limited to 512 KiB. Unknown fields or an invalid entry cause
the entire response to be rejected.

## Add or update an entry

1. Prepare an existing package with a stable ID and published `plugin.json` / `main.js`.
2. Set installUrl to the selected directory; a monorepo root URL will not locate a nested package.
3. Match the ID, name, version, and platforms to the manifest. Preserve `addedAt` during updates.
4. Update the entry and catalog `updatedAt` fields; run `pnpm test` and `pnpm test:plugin-sdk`.
5. Publish to `main`, then select Refresh catalog in Player.

The current `tests/packages.test.mjs` checks this repository's entries and known
package directories. Add each new local plugin to the test's `folders` array.
Player supports catalog links to other public repositories, but this repository's
local consistency check currently expects its own packages.

External packages with `player.*` IDs or `native.discord` / `native.soulseek`
permissions are rejected: Discord and Soulseek already ship with Player.
The Android example connects through an installed APK and Find APK plugins;
it is not listed in this JSON catalog.

## Fetching and installation

The list refreshes when the section opens and through Refresh catalog. There is
no bundled copy, disk cache, or offline fallback. On failure, the previous list
is cleared and a toast appears; file and direct-URL installation remain available.

After Install, Player validates the actual manifest and matching ID, then presents
permissions and a fingerprint. Catalog metadata does not grant permissions,
authenticate authors, or automatically update installed code. Updating bundled
packages through the submodule is a separate operation: see
[delivery](tooling.md#player-integration).
