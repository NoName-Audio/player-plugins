# Hello Player

Обычный устанавливаемый пакет `example.hello` для desktop и Android.
Подходит как основа нового плагина: не требует native-кода или внешнего сервиса.

## Файлы

| Файл | Назначение |
| --- | --- |
| [plugin.json](plugin.json) | Метаданные, картинка, права и вклады |
| [main.js](main.js) | Чтение настроек, подписка на playback, UI и Save action |

Manifest регистрирует страницу `home`, настройки `preferences` и виджет `status`
в `player.status`. При активации код читает `settings.get` и подписывается на
playback. Страница и настройки показывают название играющего трека и поле Greeting;
Save сохраняет greeting через ревизию настроек. Виджет показывает название трека
или Nothing playing. Права: `ui`, `settings`, `playback.read`.

## Запуск

Из корня player-plugins:

```sh
pnpm plugins:pack examples/hello /tmp/hello.player-plugin
```

В Player: Settings → Plugins → выбор файла → подтверждение прав → включение.
Либо установите по ссылке
`https://github.com/verncat/player-plugins/tree/main/examples/hello`.

Для собственного пакета скопируйте папку и измените `id`, `name`, автора и ссылки
в manifest. Регистрация вклада в `plugin.json` должна соответствовать `viewId` /
`action` в `main.js`. Подробности: [создание плагина](../../docs/development.md).

[Документация](../../docs/README.md) · [SDK](../../docs/sdk.md)
