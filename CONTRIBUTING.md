# Submit your plugin

To add your plugin to Player's catalog, open a pull request against `main` in
[NoName-Audio/player-plugins](https://github.com/NoName-Audio/player-plugins), adding an entry
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

## Request or add a new hook

Plugins can use only the hooks and attachment targets explicitly exposed by Player.
Adding a target name to `plugin.json` does not create a hook: an unknown attachment
target renders nothing. See the [available targets](docs/attachment-targets.md) and
the [attachment contract](docs/sdk.md#component-attachments-optional-api-2-extension)
before proposing another one.

If the hook you need is missing, open an issue or pull request in
[NoName-Audio/player](https://github.com/NoName-Audio/player), not just a catalog PR
here. Describe the plugin use case, where the hook should run, the context data it
needs, and the expected behavior when the plugin is disabled or returns no UI.
Never expose secrets, component instances, or unrestricted host access as context.

For a new before/after component attachment target:

1. In Player, follow `src/plugins/AlbumCard.ts`: register the component with
   `withPluginAttachments`, choose a stable, case-sensitive public target name,
   and project only the serializable context needed by plugins. Use the wrapped
   component at the intended application call sites; registration alone is not enough.
2. Check the surrounding layout: attachments render as siblings, not inside an
   extra wrapper. Component refs, multi-root components, transitions, KeepAlive,
   and Teleport require explicit handling; do not assume the wrapper is transparent.
3. Verify the target with an actual native plugin: before/after placement, context
   updates, actions, `null` output, disable/unmount, and rejection of stale results.
   Keep existing permission and lifecycle checks intact.
4. Run `pnpm attachments:docs` from the Player repository to regenerate the target
   reference. Include the generated documentation in the review; do not edit its
   target list by hand. Player release automation proposes the published reference
   update to this repository through a separate pull request.
5. Update the plugin to declare the target and handle `renderAttachment`. Link the
   Player PR and state which Player release is required. Existing strict API 2 hosts
   may reject manifests containing `attachments`; API 2 alone is not a compatibility
   guarantee. Publish the dependent plugin only when the required host is available.

For a new lifecycle callback, event, or host capability rather than a UI attachment,
propose the request/response contract, permissions, and compatibility behavior in
the Player PR first. Coordinate the host implementation with SDK changes and the
desktop and Android runtimes, update the relevant documentation and examples,
and test the supported platforms. A plugin-only change cannot add a host API.
