# Soulseek

Встроенный пакет `player.soulseek`: поиск музыки, предпрослушивание,
сохранение в библиотеку и поиск замены трека. Платформы: macOS, Windows, Linux, Android.

## Файлы

| Файл | Назначение |
| --- | --- |
| [plugin.json](plugin.json) | Права, настройки, поиск, track-action, внутренняя страница и player-widget |
| [main.js](main.js) | UI подключения, поиск, курсоры результатов, обложки и намерения загрузки |
| [assets/README.md](assets/README.md) | Происхождение логотипа |
| [assets/soulseek.png](assets/soulseek.png) | Исходный PNG, встроенный data URL в manifest |

## Встраивание в плеер

| Вклад | Назначение |
| --- | --- |
| `connection` / `settings` | Имя пользователя и пароль |
| `search` / `search` | Провайдер общего поиска |
| `replacement` / `page` | Внутренняя страница поиска другой версии, скрытая из бокового меню |
| `find-replacement` / `track-action` | Действие Find another version в контекстном меню трека |
| `buffer` / `player-widget` | Состояние Soulseek в `player.status` |

`main.js` вызывает API `soulseek.*`, отслеживает события передач и передаёт
воспроизведение основному координатору Player. Намерения preview/save сохраняются
в настройках с поколением активации, чтобы восстановить работу после reload Worker.
Пароль хранится через `secrets`, а не обычный settings JSON.

Соединение с сетью Soulseek и загрузки реализованы в
[Rust-адаптере Player](https://github.com/verncat/player/blob/feat/plugins/src-tauri/src/plugins/soulseek.rs).
Пакет запрашивает `ui`, `settings`, `secrets`, `playback.read`, `playback.control`,
`native.soulseek`, `library.import`, `library.replace`.

## Проверка и обновление

Из корня player-plugins запустите `pnpm test`. В репозитории Player `pnpm test:plugins`
проверяет поиск, восстановление намерений, preview/import и переход из меню трека.
Живое соединение требует отдельной проверки с учётной записью Soulseek.
Пакет встроенный: обновляется через [submodule Player](../docs/tooling.md#связь-с-player),
а не установкой внешнего `.player-plugin` с тем же зарезервированным ID.

[Документация](../docs/README.md) · [SDK](../docs/sdk.md)
