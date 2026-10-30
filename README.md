# Player plugins

Пакеты расширений, SDK, примеры и каталог для [Player](https://github.com/verncat/player).

| Путь | Содержимое |
| --- | --- |
| `catalog.json` | Список устанавливаемых расширений, который Player получает по HTTPS |
| `discord-rpc/` | Пакет Discord Rich Presence для desktop |
| `soulseek/` | Поиск, предпрослушивание, загрузка и замена треков через Soulseek |
| `examples/hello/` | Устанавливаемый JavaScript-пример |
| `examples/android-native/` | Самостоятельный Android APK с Kotlin и Rust JNI |
| `sdk.d.ts` | Типы API плагина, версия 1 |
| `scripts/` | Упаковка JavaScript-пакета и сборка Rust для Android |

## Разработка

Нужны Node.js 20+ и pnpm. Исходники плеера для упаковки JavaScript-плагина не нужны.

```sh
git clone https://github.com/verncat/player-plugins.git
cd player-plugins
pnpm install
pnpm test
pnpm test:plugin-sdk
pnpm plugins:pack examples/hello /tmp/hello.player-plugin
```

Установите файл через Settings → Plugins либо вставьте ссылку:
`https://github.com/verncat/player-plugins/tree/main/examples/hello`.
[Руководство разработчика](docs/development.md) описывает manifest, разрешения и API.
[Android-пример](examples/android-native/README.md) собирается собственным Gradle wrapper
из этой репы и не требует Gradle-проекта Player.

## Каталог

Player запрашивает `https://raw.githubusercontent.com/verncat/player-plugins/main/catalog.json`.
Репозиторий должен быть публичным. Коннектор, авторизация и кеш каталога не используются.

Для публикации расширения добавьте его папку с `plugin.json` и `main.js`, затем запись
в `catalog.json`: ID, имя, описание, repository, installUrl, дата добавления и платформы.
`installUrl` указывает на папку плагина в `main` или на готовый `.player-plugin`.
Увеличьте `updatedAt` каталога и изменённой записи, запустите проверки и запушьте в `main`.
Список обновляется в Player без новой сборки приложения. Каталог не выдаёт разрешения:
при установке Player проверяет настоящий пакет и запрашивает подтверждение.

Discord и Soulseek используют зарезервированные `player.*` ID и поставляются с Player.
Их исходники находятся здесь, но они не включены в каталог внешних пакетов.
Android APK устанавливается через Android, после чего подключается через Find APK plugins.

## Связь с Player

В репозитории Player папка `plugins/` — git submodule этой репы, закреплённый на
конкретном коммите. Сборка включает Discord/Soulseek из этой закреплённой версии;
обновление удалённого каталога не меняет встроенный код. Runtime, UI renderer,
проверка пакетов, Rust-адаптеры и интеграционные тесты остаются в Player.

Чтобы обновить встроенные пакеты, из репозитория Player выполните:

```sh
git submodule update --init -- plugins
git -C plugins fetch origin main
git -C plugins checkout --detach origin/main
git add plugins
git commit -m "Update bundled plugins"
```

Спецификации хоста: [docs/plugins/scope](https://github.com/verncat/player/tree/feat/plugins/docs/plugins/scope).
