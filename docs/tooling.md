# Инструменты, проверки и поставка

[Документация](README.md) · [Структура файлов](structure.md)

## Команды player-plugins

Все команды в таблице выполняются из корня этой репы.

| Команда | Что выполняет |
| --- | --- |
| `pnpm install --frozen-lockfile` | Устанавливает закреплённые зависимости инструментов |
| `pnpm test` | Упаковывает четыре пакета, проверяет синтаксис JS и согласованность записей каталога |
| `pnpm test:plugin-sdk` | Проверяет типы в `sdk.d.ts`, не `main.js` всех плагинов |
| `pnpm plugins:pack <folder> [output]` | Формирует JSON-пакет из двух файлов |
| `pnpm plugins:android:build` | Собирает debug APK Android-примера через Gradle wrapper |

[package-plugin.mjs](../scripts/package-plugin.mjs) читает `plugin.json` и `main.js`,
формирует `{manifest, script}` и проверяет лимит 4 MiB. Если output не указан,
создаёт `<folder>/<id>-<version>.player-plugin`. Это упаковка, а не компиляция
или полная валидация схемы/прав Player.

[build-android-plugin.mjs](../scripts/build-android-plugin.mjs) вызывается Gradle
с папкой примера и списком ABI через запятую. Он выбирает NDK из `ANDROID_NDK_HOME`
либо SDK из `ANDROID_HOME` / `ANDROID_SDK_ROOT`, запускает Cargo с `--locked --release`
и копирует `.so` в `build/generated/jniLibs/<abi>/`. Сборка APK и assets остаётся Gradle.
Подробности: [Android-пример](../examples/android-native/README.md).

## Автоматические проверки

[check.yml](../.github/workflows/check.yml) запускается на push и pull request:
Node.js 22, pnpm 10, установка по lockfile, `pnpm test`, `pnpm test:plugin-sdk`.
APK и живые подключения к Discord/Soulseek этот workflow не собирает и не проверяет.

[packages.test.mjs](../tests/packages.test.mjs) использует реальные исходники пакетов.
Он проверяет, что упаковщик работает из другой рабочей папки, ID уникальны,
скрипты синтаксически корректны, а catalog installUrl ведёт к существующей локальной
папке с согласованными ID, именем, версией и платформами.

Тесты этой репы не запускают Worker или native gateway. В репозитории **Player**
выполняются интеграционные проверки, например:

```sh
pnpm test:plugins
cargo test --manifest-path src-tauri/Cargo.toml --lib
```

## Связь с Player

В Player `plugins/` — git submodule, закреплённый на конкретном коммите этой репы.
Соседний checkout `../player-plugins` и checkout `Player/plugins/` независимы:
изменение файлов в первом автоматически не меняет второй.

Для первоначальной сборки выполните из корня **Player**:

```sh
git submodule update --init -- plugins
```

Чтобы включить новую версию встроенных пакетов в сборку, сначала опубликуйте её
в этой репе, затем из корня **Player**:

```sh
git -C plugins fetch origin main
git -C plugins checkout --detach origin/main
git add plugins
git commit -m "Update bundled plugins"
```

После этого проверьте Player и опубликуйте его коммит обычным способом. Его release
workflow получает закреплённый submodule; изменения `main` этой репы не подменяют
код уже собранного приложения. Обновление только `catalog.json` становится доступно
без обновления gitlink и сборки Player.

Runtime, UI renderer и native-адаптеры остаются в Player. Пакеты `player.discord-rpc`
и `player.soulseek` нельзя переустановить как обычные внешние `.player-plugin`.
