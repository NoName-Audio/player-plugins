# Submit your plugin

To add your plugin to Player's catalog, open a pull request against `main` in
[verncat/player-plugins](https://github.com/verncat/player-plugins), adding an entry
to the `plugins` array in [catalog.json](catalog.json).

Your plugin can be developed and published in **your own public repository**. You do
not need to move its source code into this repository. Set `repository` to your
repository URL and `installUrl` to the published, compiled `.player-plugin` package,
for example an asset in your own GitHub Releases. A source-only repository or
`plugin.json` URL cannot be installed as a desktop native package.

## Pull request steps

1. Build and publish your native package for the platforms you advertise.
2. Fork this repository and add your entry to `catalog.json`. Keep existing entries.
3. Use your plugin's actual manifest ID, name, version and platforms. Include links
   to your repository and installable package. Add author information and tags if useful.
4. Set `addedAt` and `updatedAt` to the submission date (`YYYY-MM-DD`) and update the
   catalog's top-level `updatedAt`. For updates, preserve the original `addedAt`.
5. Test installing the package from its public URL in Player, then open a pull request
   against this repository's `main` branch. Include a short description and the
   platforms you tested. Maintainers review and merge the catalog change.

Example entry (replace the example metadata, URLs and dates):

```json
{
  "id": "com.example.my-plugin",
  "name": "My Plugin",
  "description": "A short description of what the plugin adds to Player.",
  "version": "1.0.0",
  "author": { "name": "Your name", "url": "https://github.com/your-name" },
  "repository": "https://github.com/your-name/my-plugin",
  "installUrl": "https://github.com/your-name/my-plugin/releases/download/v1.0.0/my-plugin.player-plugin",
  "image": "https://raw.githubusercontent.com/your-name/my-plugin/main/assets/icon.png",
  "platforms": ["macos", "windows", "linux"],
  "addedAt": "2026-10-31",
  "updatedAt": "2026-10-31",
  "tags": ["music"]
}
```

All catalog links must use public HTTPS URLs. Catalog images link to files; do not
embed base64. Your source `plugin.json` can use a local image path instead.
For Android, add `android` to `platforms` and `androidApkUrl` pointing to your published
standalone APK. Player downloads it through Android's installer and connects it with
Find APK plugins.

For later releases, publish the new package in your repository and submit another
pull request updating the catalog's version, install link and update dates. Catalog
changes do not automatically update packages already installed by users.

See [catalog format](docs/catalog.md) and [native plugin development](docs/development.md).
