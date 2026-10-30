# SDK и декларативный интерфейс

[Документация](README.md) · [Создать плагин](development.md)

Глобальные типы находятся в [sdk.d.ts](../sdk.d.ts). Файл не нужно импортировать
в исполняемый `main.js`: `registerPlugin` и контекст создаёт Player.
[tsconfig.sdk.json](../tsconfig.sdk.json) проверяет декларации, а не код всех плагинов.

## Жизненный цикл

Один `registerPlugin(factory)` регистрирует синхронную factory. Она получает `ctx`
и возвращает обработчики; каждый обработчик необязателен.

| Обработчик | Когда вызывается |
| --- | --- |
| `activate()` | После создания экземпляра, может вернуть Promise |
| `render({viewId, context})` | Когда хост запрашивает UI конкретного вклада |
| `action({action, payload, fields, context})` | После кнопки, переключателя или действия трека |
| `search({providerId, query, cursor, limit})` | Для страницы результатов поиска |
| `renderSearch({ids})` | Для обновления уже найденных строк; `null` сохраняет прежние |
| `deactivate()` | При штатном отключении; освобождает ресурсы экземпляра |

При отключении отменяется `ctx.signal`, хост завершает Worker и отклоняет дальнейшие
capability-вызовы. Сохранённые настройки и native-операции не зависят от открытой панели;
временные переменные Worker после его пересоздания нужно восстановить самостоятельно.

## Контекст

| API | Назначение |
| --- | --- |
| `ctx.pluginId`, `ctx.generation` | ID и поколение активации |
| `ctx.signal` | AbortSignal времени жизни экземпляра |
| `ctx.call(method, args)` | Асинхронный вызов API хоста с проверкой прав |
| `ctx.on(topic, handler)` | Native-события своего плагина; возвращает unsubscribe |
| `ctx.playback.subscribe(handler)` | Playback snapshots; возвращает unsubscribe |
| `ctx.playback.pause/resume/stop/seek` | Управление transport, время в секундах |
| `ctx.playback.play`, `ctx.playback.adopt` | Воспроизведение ресурса / принятие импортированного трека |
| `ctx.ui.invalidate()` | Перечитать дерево UI и отображаемые результаты |
| `ctx.ui.navigate(pageId, context)` | Открыть собственную зарегистрированную страницу |
| `ctx.android.call(method, args)` | Вызов backend подключённого Android APK |
| `ctx.log(message)` | Локальная диагностика |

Snapshot содержит `title`, `artist`, `album`, `position`, `duration`, `playing`,
необязательный `resource`. Позиция и длительность — секунды. Для подписки нужно
`playback.read`; для управления — `playback.control`.

## Настройки и секреты

`settings.get` возвращает `{revision, values}`. В `settings.update` передаются
ожидаемая `revision` и полное новое `values`; при конфликте нужно перечитать данные.
Настройки относятся к конкретному ID и хранятся на устройстве в Player.

```js
const settings = await ctx.call('settings.get');
await ctx.call('settings.update', {
  revision: settings.revision,
  values: { ...settings.values, greeting: 'Hello' }
});
ctx.ui.invalidate();
```

Этот фрагмент выполняется внутри асинхронного обработчика с правом `settings`.
Для секретов доступны `secrets.has`, `secrets.get`, `secrets.set`, `secrets.delete`
с ключом `key` (`set` также получает `value`); требуется `secrets`.
Пароли не помещайте в обычные настройки, логи и UI. Наличие секрета проверяйте через
`has`, не загружая значение только ради отображения формы.

## Вклады plugin.json

| `kind` | Куда встраивается | Связанный обработчик |
| --- | --- | --- |
| `page` | Страница и пункт бокового меню | `render`, `viewId` равен ID вклада |
| `settings` | Раздел настроек | `render` |
| `search` | Блок общего поиска | `search`, `renderSearch` |
| `track-action` | Контекстное меню трека | `action`, `payload.track` |
| `player-widget` | `player.actions` или `player.status` | `render` |

Каждому вкладу нужны `id`, `kind`, `label`; доступны `description`, `icon`, `order`.
Локальный ID — до 64 символов `[a-z0-9-]`. Для `player-widget` обязателен `slot`.
Для `page` можно задать `showInMenu: false`, сохранив внутреннюю навигацию через
`ctx.ui.navigate()`. У остальных видов вкладов это поле запрещено.
Любые вклады требуют права `ui`.

## Дерево UI

| `type` | Основные поля |
| --- | --- |
| `group` | `title?`, `children?` |
| `text` | `text`, `tone?` (`error` для ошибки) |
| `input` | `key`, `label`, `value?`, `inputType?` (`text` / `password`) |
| `button` | `label`, `action`, `payload?`, `disabled?` |
| `toggle` | `label`, `value`, `action`, `payload?`, `disabled?` |
| `results` | `items` — строки результатов |

Поля формы приходят в `action.fields` по `key`; состояние после сохранения
обновляйте через `ctx.ui.invalidate()`. Хост применяет тему и общие стили Player,
показывает ожидание на нажатой кнопке и блокирует повторное действие.
HTML, CSS и Vue-компоненты в дереве не передаются. Глубина ограничена 12 уровнями,
число детей узла — 100, число строк `results` — 1000.

Поиск возвращает `{items, nextCursor}`. Строка: `{id, title, subtitle?, coverHash?,
status?, actions?}`; действие строки получает `payload.resourceId`, а не track ID
библиотеки. Формат непрозрачного курсора определяет плагин.

Рабочие образцы: [Hello](../examples/hello/main.js), [Soulseek](../soulseek/main.js).
Полный контракт хоста: [SDK scope](https://github.com/verncat/player/blob/feat/plugins/docs/plugins/scope/sdk-ui-scope.md).
