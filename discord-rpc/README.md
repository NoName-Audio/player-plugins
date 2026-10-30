# Discord RPC

Встроенный пакет `player.discord-rpc`: показывает текущий трек и исполнителя
в Discord Rich Presence. Поддерживает macOS, Windows и Linux с Discord desktop.

## Файлы

| Файл | Назначение |
| --- | --- |
| [plugin.json](plugin.json) | ID, платформы, разрешения и вклад `settings/general` |
| [main.js](main.js) | Подписка на playback, фильтрация повторов и вызовы `discord.update` |
| [assets/README.md](assets/README.md) | Источник логотипа и инструкция генерации PNG |
| [assets/discord.svg](assets/discord.svg) | Векторный исходник логотипа |
| [assets/discord.png](assets/discord.png) | Растр, встроенный data URL в manifest |

## Поведение

При активации `main.js` подписывается на состояние воспроизведения. Он отправляет
изменения трека и заметные изменения позиции в API хоста, объединяет ожидающие
обновления и очищает activity при паузе. Ошибка native-вызова появляется в разделе
настроек плагина. Изменение PNG отдельно от manifest не обновляет картинку пакета.

Права: `ui`, `playback.read`, `settings`, `native.discord`.
IPC-соединение с Discord и переподключение реализованы в
[Rust-адаптере Player](https://github.com/verncat/player/blob/feat/plugins/src-tauri/src/plugins/discord.rs).

## Проверка и обновление

Из корня player-plugins запустите `pnpm test`. Worker-сценарии проверяются через
`pnpm test:plugins` в репозитории Player; живую activity проверяют с запущенным Discord.
Пакет встроенный, с зарезервированным ID: упаковка не делает его доступным для
обычной внешней установки. Обновление — через [submodule в Player](../docs/tooling.md#связь-с-player).

[Документация](../docs/README.md) · [Структура репозитория](../docs/structure.md)
