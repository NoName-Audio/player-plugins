# Структура репозитория

[Документация](README.md) · [Разработка плагина](development.md)

```text
player-plugins/
├── README.md                     # точка входа и быстрый старт
├── catalog.json                  # публичный список расширений
├── package.json                  # команды pnpm и зависимости инструментов
├── pnpm-lock.yaml                # закреплённая версия TypeScript
├── sdk.d.ts                      # глобальные типы API 1
├── tsconfig.sdk.json             # проверка деклараций SDK
├── .gitignore                    # исключения для зависимостей и результатов сборки
├── .github/workflows/check.yml   # проверки пакетов и SDK при push / pull request
├── docs/                         # общие руководства
│   ├── README.md                 # навигация по документации
│   ├── structure.md              # этот документ
│   ├── development.md            # создание пакета и формат manifest
│   ├── sdk.md                    # API, lifecycle и декларативный UI
│   ├── catalog.md                # публикация списка расширений
│   └── tooling.md                # команды, проверки и связь с Player
├── discord-rpc/                  # встроенный пакет Player
│   ├── README.md                 # поведение и карта файлов пакета
│   ├── plugin.json               # ID, платформы, разрешения, раздел настроек
│   ├── main.js                   # playback-события → Discord API хоста
│   └── assets/
│       ├── README.md             # источник логотипа и генерация PNG
│       ├── discord.svg           # векторный исходник
│       └── discord.png           # логотип, встроенный в manifest как data URL
├── soulseek/                     # встроенный пакет Player
│   ├── README.md                 # поиск, загрузки, предпрослушивание, замена
│   ├── plugin.json               # права и вклады в настройки, поиск и меню трека
│   ├── main.js                   # UI и координация native-операций
│   └── assets/
│       ├── README.md             # источник логотипа
│       └── soulseek.png          # логотип, встроенный в manifest как data URL
├── examples/
│   ├── hello/
│   │   ├── README.md             # запуск и поведение примера
│   │   ├── plugin.json           # обычный внешний пакет example.hello
│   │   └── main.js               # настройки, страница и события воспроизведения
│   └── android-native/
│       ├── README.md             # сборка, подключение APK и проверки
│       ├── plugin.json           # manifest плагина, встраиваемый в APK
│       ├── main.js               # UI и ctx.android.call()
│       ├── build.gradle.kts      # сборка APK, JSON assets и Rust JNI
│       ├── settings.gradle.kts   # Android/Kotlin Gradle plugins и repositories
│       ├── gradle.properties     # настройки Gradle и AndroidX
│       ├── proguard-rules.pro    # сохранение reflection/JNI entrypoint
│       ├── src/main/
│       │   ├── AndroidManifest.xml
│       │   └── java/dev/verncat/player/example/nativeplugin/NativePlugin.kt
│       └── native/
│           ├── Cargo.toml        # Rust cdylib player_native_example
│           ├── Cargo.lock        # закреплённые JNI/JSON зависимости
│           └── src/lib.rs        # JNI entrypoint, echo и unit test
├── scripts/
│   ├── package-plugin.mjs        # plugin.json + main.js → .player-plugin
│   └── build-android-plugin.mjs  # Cargo + NDK → .so для выбранных ABI
├── tests/packages.test.mjs       # упаковка всех пакетов и согласованность каталога
├── gradlew / gradlew.bat         # Gradle wrapper для Android-примера
└── gradle/wrapper/
    ├── gradle-wrapper.jar
    └── gradle-wrapper.properties # закреплённый дистрибутив Gradle
```

## Структура обычного пакета

Минимум — `plugin.json` и `main.js` в одной папке. Manifest описывает пакет,
`main.js` содержит весь исполняемый код и один вызов `registerPlugin`.
Подробнее: [создание пакета](development.md).

`README.md` объясняет поведение конкретного плагина. В `assets/` хранятся исходники
изображений и сведения об их происхождении. Упаковщик читает только два обязательных
файла: изображения и другие ресурсы автоматически в пакет не добавляются.
Поля `image` с относительным путём не поддерживаются; используйте HTTP(S) или data URL.
Замена PNG в `assets/` сама по себе не обновит встроенную картинку в `plugin.json`.

Новый плагин создавайте в отдельной папке; `examples/` предназначена для образцов.
Папка и ID — разные сущности: ID задаётся в manifest и сохраняется при обновлении.
Добавление папки не добавляет запись в [каталог](catalog.md) автоматически.

## Исходники и результаты сборки

| Результат | Где появляется | Хранение |
| --- | --- | --- |
| Установочные npm-зависимости | `node_modules/` | Исключены из Git |
| Упакованный JS-плагин | Указанный output либо `<folder>/<id>-<version>.player-plugin` | Исключён из Git |
| Rust Android build | `examples/android-native/native/target/` | Исключён из Git |
| Скопированные JNI-библиотеки | `examples/android-native/build/generated/jniLibs/<abi>/` | Исключены из Git |
| JSON manifest/script для APK | `examples/android-native/build/generated/pluginAssets/player-plugin.json` | Генерируется Gradle |
| Debug APK | `examples/android-native/build/outputs/apk/debug/android-native-debug.apk` | Исключён из Git |
| Gradle cache / SDK path | `.gradle/`, `local.properties` | Исключены из Git |

`Cargo.lock`, `pnpm-lock.yaml` и Gradle wrapper хранятся в Git для воспроизводимой сборки.

## Граница с Player

Эта репа хранит пакеты, а Player — Worker runtime, UI renderer, проверку manifest,
установку, хранилища и native gateway. Поэтому `src/` и `src-tauri/` плеера здесь нет.
Rust в `examples/android-native/native/` принадлежит отдельному APK; встроенные
Rust-адаптеры Discord/Soulseek находятся в Player.

В Player эта репа подключена как submodule `plugins/`. Например,
`player-plugins/soulseek/main.js` доступен в его checkout как `Player/plugins/soulseek/main.js`.
Как обновлять закреплённый коммит: [инструменты и поставка](tooling.md#связь-с-player).
