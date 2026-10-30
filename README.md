# Player plugins

Исходники расширений для [Player](https://github.com/verncat/player): встроенные
Discord RPC и Soulseek, устанавливаемый JavaScript-пример, Android APK-пример,
SDK и публичный каталог.

## Что где лежит

| Путь | Назначение |
| --- | --- |
| [discord-rpc/](discord-rpc/README.md) | Пакет Discord Rich Presence для desktop |
| [soulseek/](soulseek/README.md) | Пакет поиска, предпрослушивания и загрузки музыки |
| [examples/hello/](examples/hello/README.md) | Образец обычного устанавливаемого плагина |
| [examples/android-native/](examples/android-native/README.md) | Отдельный Android APK с Kotlin и Rust JNI |
| [sdk.d.ts](sdk.d.ts) | Типы API плагинов, версия 1 |
| [catalog.json](catalog.json) | Список расширений для Settings → Plugins |
| [scripts/](docs/tooling.md) | Упаковка JavaScript и сборка Rust для Android |
| [docs/](docs/README.md) | Руководства разработчика и сопровождающего |

Полное дерево с назначением файлов: [структура репозитория](docs/structure.md).

## Начать работу

Нужны Node.js 20+ и pnpm. Для упаковки JavaScript-плагина исходники Player не нужны.
Все команды ниже выполняются из корня **player-plugins**.

```sh
git clone https://github.com/verncat/player-plugins.git
cd player-plugins
pnpm install
pnpm test
pnpm test:plugin-sdk
pnpm plugins:pack examples/hello /tmp/hello.player-plugin
```

Установите файл через Settings → Plugins или используйте ссылку
`https://github.com/verncat/player-plugins/tree/main/examples/hello`.

## Документация

- [Создать плагин](docs/development.md): папка, manifest, код, упаковка и установка.
- [SDK и UI](docs/sdk.md): жизненный цикл, вызовы API и элементы интерфейса.
- [Опубликовать в каталоге](docs/catalog.md): формат JSON и добавление записи.
- [Инструменты и связь с Player](docs/tooling.md): команды, проверки, submodule и обновления.

Player загружает каталог из
`https://raw.githubusercontent.com/verncat/player-plugins/main/catalog.json`.
Discord/Soulseek поставляются вместе с Player из закреплённого коммита этой репы;
runtime и Rust-адаптеры находятся в репозитории плеера.
