# Плагины Player

Система добавляет функции плеера через устанавливаемые JavaScript-пакеты.
Каждый пакет работает в отдельном Web Worker. UI описывается данными и рисуется
компонентами Player; доступ к библиотеке, источникам, настройкам и native-коду
проходит через API хоста. `apiVersion: 1` — текущий контракт.

Discord RPC и Soulseek поставляются как встроенные пакеты с отдельными Rust-
адаптерами. Их можно отключить; обычная библиотека и аудиодвижок работают без них.
Новый JavaScript-плагин устанавливается без пересборки Player на desktop и Android.
На Android добавлен эксперимент самостоятельных APK-плагинов с Kotlin/Rust:
загрузчик подключает их код внутри процесса Player. Подробности и ограничения —
в [Android native scope](https://github.com/verncat/player/blob/feat/plugins/docs/plugins/scope/android-native-scope.md).
На desktop новые native-адаптеры пока добавляются в сборку приложения.

## Документы по областям

Все scope-документы описывают одну систему и дополняют этот readme:

| Область | Документ |
| --- | --- |
| Границы и архитектура | [system-scope](https://github.com/verncat/player/blob/feat/plugins/docs/plugins/scope/system-scope.md) |
| Пакет, установка и обновление | [package-scope](https://github.com/verncat/player/blob/feat/plugins/docs/plugins/scope/package-scope.md) |
| Каталог плагинов из репозитория | [catalog-scope](https://github.com/verncat/player/blob/feat/plugins/docs/plugins/scope/catalog-scope.md) |
| Runtime, жизненный цикл и сообщения | [runtime-scope](https://github.com/verncat/player/blob/feat/plugins/docs/plugins/scope/runtime-scope.md) |
| SDK и UI | [sdk-ui-scope](https://github.com/verncat/player/blob/feat/plugins/docs/plugins/scope/sdk-ui-scope.md) |
| Источники, передачи и библиотека | [media-scope](https://github.com/verncat/player/blob/feat/plugins/docs/plugins/scope/media-scope.md) |
| Хранение и права | [storage-security-scope](https://github.com/verncat/player/blob/feat/plugins/docs/plugins/scope/storage-security-scope.md) |
| Discord RPC | [discord-scope](https://github.com/verncat/player/blob/feat/plugins/docs/plugins/scope/discord-scope.md) |
| Soulseek | [soulseek-scope](https://github.com/verncat/player/blob/feat/plugins/docs/plugins/scope/soulseek-scope.md) |
| Проверки и поставка | [delivery-scope](https://github.com/verncat/player/blob/feat/plugins/docs/plugins/scope/delivery-scope.md) |
| Эксперимент Android APK / Rust | [android-native-scope](https://github.com/verncat/player/blob/feat/plugins/docs/plugins/scope/android-native-scope.md) |

Описание текущей основной
архитектуры находится в [DESIGN.md](https://github.com/verncat/player/blob/feat/plugins/DESIGN.md).

## Быстрый старт разработчика

Рабочий пример: [plugins/examples/hello](../examples/hello).
Самостоятельные типы для редактора: [plugins/sdk.d.ts](../sdk.d.ts).
Создайте два файла: `plugin.json` и `main.js`. Весь JavaScript и зависимости
должны быть собраны в один script без внешних import/require. Player не поставляет
плагину Node.js, npm, Vue или доступ к DOM/Tauri.

```json
{
  "id": "example.my-plugin",
  "name": "My plugin",
  "version": "1.0.0",
  "apiVersion": 1,
  "description": "Example plugin",
  "image": "https://example.com/my-plugin.png",
  "author": {"name": "Your name", "url": "https://example.com"},
  "repository": "https://github.com/example/my-plugin",
  "homepage": "https://example.com/my-plugin",
  "platforms": ["macos", "windows", "linux", "android"],
  "permissions": ["ui"],
  "contributions": [
    {"id": "home", "kind": "page", "label": "My plugin"}
  ]
}
```

`image` — необязательная картинка для карточки плагина. Можно указать HTTP(S)
URL или встроить PNG/JPEG/WebP/GIF строкой `data:image/png;base64,...` (до 256 KiB).
Встроенная картинка не требует сети; рабочий пример Hello Player содержит такую.
При отсутствии картинки или ошибке загрузки отображается значок плагина.

```js
registerPlugin((ctx) => ({
  activate() {},
  render({viewId, context}) {
    return {type: 'text', text: 'Hello from my plugin'};
  },
  async action({action, payload, fields, context}) {},
  deactivate() {}
}));
```

Соберите пакет из корня репозитория:

```sh
pnpm plugins:pack examples/hello /tmp/hello.player-plugin
```

Откройте Settings → Plugins, выберите `.player-plugin` или вставьте публичную
GitHub-ссылку в Load from URL. Проверьте автора,
запрошенные права и fingerprint, установите и включите. Страница появится в
боковой панели. Для обновления отключите плагин и установите новую версию с тем
же ID. Настройки сохраняются. Встроенные пакеты обновляются вместе с приложением.

Для внутренней страницы укажите `showInMenu: false` во вкладе `kind: "page"`.
Она не добавляется в боковое меню и открывается через `ctx.ui.navigate(id, context)`.
Так Soulseek открывает поиск замены из контекстного меню трека; обычный поиск
Soulseek остаётся в общем Search. По умолчанию страницы видны в навигации.

Репозиторий должен содержать plugin.json и main.js в выбранной папке.
Например, `https://github.com/owner/plugin` или
`https://github.com/owner/repo/tree/main/plugins/my-plugin`. Поддержаны также
прямые HTTPS-ссылки на package и plugin.json. Коннектор и вход в GitHub не нужны;
приватные репозитории требуют отдельного решения. Для GitHub сохраняется
конкретный загруженный commit. Подробный контракт — в
[package scope](https://github.com/verncat/player/blob/feat/plugins/docs/plugins/scope/package-scope.md#загрузка-по-ссылке).

## Карта реализации

Settings → Plugins содержит каталог расширений из
[plugins/catalog.json](../catalog.json) в репозитории player-plugins. Здесь
можно искать по названию, автору и тегам и нажать Install. Установка
использует обычный просмотр пакета и его разрешений. Список загружается только
из сети, без встроенной копии и кеша. Как добавить запись и опубликовать индекс — в
[catalog scope](https://github.com/verncat/player/blob/feat/plugins/docs/plugins/scope/catalog-scope.md).

| Компонент | Исходники |
| --- | --- |
| Проверка manifest/package | [model.rs](https://github.com/verncat/player/blob/feat/plugins/src-tauri/src/plugins/model.rs) |
| Реестр, настройки, поколения и handles | [store.rs](https://github.com/verncat/player/blob/feat/plugins/src-tauri/src/plugins/store.rs) |
| Загрузка из репозитория/по URL | [remote.rs](https://github.com/verncat/player/blob/feat/plugins/src-tauri/src/plugins/remote.rs) |
| Каталог из репозитория | [catalog.rs](https://github.com/verncat/player/blob/feat/plugins/src-tauri/src/plugins/catalog.rs), [PluginCatalog.vue](https://github.com/verncat/player/blob/feat/plugins/src/plugins/components/PluginCatalog.vue) |
| Native gateway | [mod.rs](https://github.com/verncat/player/blob/feat/plugins/src-tauri/src/plugins/mod.rs) |
| Android APK loader | [android.rs](https://github.com/verncat/player/blob/feat/plugins/src-tauri/src/plugins/android.rs), [ApkPlugins.kt](https://github.com/verncat/player/blob/feat/plugins/src-tauri/gen/android/app/src/main/java/dev/verncat/player/ApkPlugins.kt) |
| Credential providers | [secrets.rs](https://github.com/verncat/player/blob/feat/plugins/src-tauri/src/plugins/secrets.rs) |
| Runtime и frontend broker | [host.ts](https://github.com/verncat/player/blob/feat/plugins/src/plugins/host.ts), [worker.js](https://github.com/verncat/player/blob/feat/plugins/src/plugins/worker.js), [messages.ts](https://github.com/verncat/player/blob/feat/plugins/src/plugins/messages.ts) |
| Публичные DTO | [types.ts](https://github.com/verncat/player/blob/feat/plugins/src/plugins/types.ts) |
| UI renderer и управление | [components](https://github.com/verncat/player/blob/feat/plugins/src/plugins/components) |
| Общий источник аудио | [plugin_sources.rs](https://github.com/verncat/player/blob/feat/plugins/src-tauri/src/plugin_sources.rs), [playback.rs](https://github.com/verncat/player/blob/feat/plugins/src-tauri/src/playback.rs) |
| Общие правила импорта | [import_paths.rs](https://github.com/verncat/player/blob/feat/plugins/src-tauri/src/import_paths.rs) |
| Встроенные плагины | [Discord](../discord-rpc), [Soulseek](../soulseek) |

## Модель доверия и платформы

Устанавливайте пакеты доверенных авторов. Worker отделяет код плагина от DOM
и основного потока интерфейса; он не является полной сетевой sandbox.
Fingerprint фиксирует содержимое выбранного пакета, но не удостоверяет автора.
Подписей издателей и фонового обновления пакетов в этой версии нет.
У APK-плагинов дополнительно фиксируются SHA-256 установленного файла и его
сертификата подписи. Это контроль просмотренной версии, а не проверка личности
автора. Native-код имеет все Android-разрешения Player и не изолирован Worker.

Пример APK: [android-native](../examples/android-native/README.md).
Сборка: `pnpm plugins:android:build`. После установки APK средствами Android
откройте Settings → Plugins → Find APK plugins, подтвердите подключение и
включите его. Обновление уже загруженной `.so` требует перезапуска Player.

Пароли хранятся через Keychain на macOS, Credential Manager на Windows,
Secret Service (`secret-tool`) на Linux и Android Keystore с зашифрованными
private preferences на Android. Отсутствие или отказ provider показывается
как ошибка; новые секреты не сохраняются открытым текстом.

Discord RPC доступен на desktop с feature `discord-rpc`; Soulseek — с feature
`soulseek`. Полная сборка включает обе feature. Сборка без них:

```sh
cargo check --manifest-path src-tauri/Cargo.toml --no-default-features --lib
```

Проверки пакетов: `pnpm test`, `pnpm test:plugin-sdk`.
Интеграционные проверки Worker, UI и native gateway выполняются в репозитории Player.
Живое соединение с Discord/Soulseek проверяется отдельно от автономных тестов.
