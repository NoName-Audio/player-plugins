# Документация player-plugins

Команды в этих документах выполняются из корня `player-plugins`, если явно
не указан репозиторий **Player**. Пути файлов также указаны от корня текущей репы.

| Задача | Документ |
| --- | --- |
| Найти исходники и понять назначение файлов | [Структура репозитория](structure.md) |
| Написать первый JavaScript-плагин | [Разработка](development.md) |
| Подключить UI, настройки и события плеера | [SDK](sdk.md) |
| Добавить расширение в список плеера | [Каталог](catalog.md) |
| Упаковать, проверить или обновить встроенные пакеты | [Инструменты и поставка](tooling.md) |
| Разобраться с Discord | [discord-rpc](../discord-rpc/README.md) |
| Разобраться с поиском, загрузкой и заменой треков | [soulseek](../soulseek/README.md) |
| Запустить минимальный JavaScript-пример | [Hello Player](../examples/hello/README.md) |
| Собрать плагин с Rust на Android | [Android native](../examples/android-native/README.md) |

Текущий контракт — **Plugin API 1**. Его типы: [sdk.d.ts](../sdk.d.ts).
Схему пакетов и права проверяет Player; упаковщик из этой репы только формирует JSON.

Детали реализации хоста и интеграционные сценарии находятся в
[scope-документах Player](https://github.com/verncat/player/tree/feat/plugins/docs/plugins/scope).
