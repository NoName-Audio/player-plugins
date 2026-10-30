# SDK and declarative UI

[Documentation](README.md) · [Create a plugin](development.md)

Global declarations live in [sdk.d.ts](../sdk.d.ts). Do not import this file into
the executable `main.js`: Player supplies `registerPlugin` and the context.
[tsconfig.sdk.json](../tsconfig.sdk.json) checks the declarations, not every plugin's code.

## Lifecycle

One `registerPlugin(factory)` call registers a synchronous factory. It receives
`ctx` and returns handlers; each handler is optional.

| Handler | Invocation |
| --- | --- |
| `activate()` | After instance creation; may return a Promise |
| `render({viewId, context})` | When the host requests a contribution's UI |
| `action({action, payload, fields, context})` | After a button, toggle, or track action |
| `search({providerId, query, cursor, limit})` | For a page of search results |
| `renderSearch({ids})` | To refresh existing rows; `null` preserves previous rows |
| `deactivate()` | During normal shutdown; releases instance resources |

Disabling a plugin aborts `ctx.signal`, terminates its Worker, and rejects further
capability calls. Stored settings and native operations are independent of an open
panel; restore transient Worker state when a new instance is created.

## Context

| API | Purpose |
| --- | --- |
| `ctx.pluginId`, `ctx.generation` | Plugin ID and activation generation |
| `ctx.signal` | Instance lifetime AbortSignal |
| `ctx.call(method, args)` | Asynchronous host API call with permission checks |
| `ctx.on(topic, handler)` | Plugin-owned native events; returns an unsubscribe function |
| `ctx.playback.subscribe(handler)` | Playback snapshots; returns an unsubscribe function |
| `ctx.playback.pause/resume/stop/seek` | Transport control; time is in seconds |
| `ctx.playback.play`, `ctx.playback.adopt` | Play a resource / adopt an imported track |
| `ctx.ui.invalidate()` | Refresh the UI tree and displayed results |
| `ctx.ui.navigate(pageId, context)` | Open one of the plugin's registered pages |
| `ctx.android.call(method, args)` | Call the connected Android APK backend |
| `ctx.log(message)` | Local diagnostics |

A snapshot contains `title`, `artist`, `album`, `position`, `duration`, `playing`,
and an optional `resource`. Position and duration use seconds. Subscriptions require
`playback.read`; playback control requires `playback.control`.

## Settings and secrets

`settings.get` returns `{revision, values}`. Pass the expected `revision` and the
complete new `values` to `settings.update`; read the latest settings after a conflict.
Settings belong to a specific plugin ID and are stored locally by Player.

```js
const settings = await ctx.call('settings.get');
await ctx.call('settings.update', {
  revision: settings.revision,
  values: { ...settings.values, greeting: 'Hello' }
});
ctx.ui.invalidate();
```

Run this snippet inside an asynchronous handler with the `settings` permission.
Secrets use `secrets.has`, `secrets.get`, `secrets.set`, and `secrets.delete` with
`key` (`set` also takes `value`); they require `secrets`.
Keep passwords out of ordinary settings, logs, and UI. Use `has` to check whether
a secret exists without loading its value just to render a form.

## plugin.json contributions

| `kind` | Placement | Handler |
| --- | --- | --- |
| `page` | A page and sidebar entry | `render`; `viewId` is the contribution ID |
| `settings` | A settings section | `render` |
| `search` | A block in global search | `search`, `renderSearch` |
| `track-action` | A track's context menu | `action`, `payload.track` |
| `player-widget` | `player.actions` or `player.status` | `render` |

Each contribution needs `id`, `kind`, and `label`; optional fields include
`description`, `icon`, and `order`. Local IDs use `[a-z0-9-]` and are limited
to 64 characters. `player-widget` requires a `slot`.
A `page` can set `showInMenu: false` while remaining accessible through
`ctx.ui.navigate()`. Other contribution kinds reject this field.
All contributions require the `ui` permission.

## UI tree

| `type` | Main fields |
| --- | --- |
| `group` | `title?`, `children?` |
| `text` | `text`, `tone?` (`error` for errors) |
| `input` | `key`, `label`, `value?`, `inputType?` (`text` / `password`) |
| `button` | `label`, `action`, `payload?`, `disabled?` |
| `toggle` | `label`, `value`, `action`, `payload?`, `disabled?` |
| `results` | `items` — result rows |

Form values arrive in `action.fields`, indexed by `key`. Call `ctx.ui.invalidate()`
to refresh state after saving. The host applies Player's theme and shared styles,
shows pending feedback on the clicked button, and blocks duplicate actions.
UI trees do not contain HTML, CSS, or Vue components. Limits are 12 levels of depth,
100 children per node, and 1000 rows in `results`.

Search returns `{items, nextCursor}`. A row is `{id, title, subtitle?, coverHash?,
status?, actions?}`; its action receives `payload.resourceId`, not a library track ID.
The plugin defines its opaque cursor format.

Working examples: [Hello](../examples/hello/main.js), [Soulseek](../soulseek/main.js).
Full host contract: [SDK scope](https://github.com/verncat/player/blob/feat/plugins/docs/plugins/scope/sdk-ui-scope.md).
