# Создание JavaScript-плагина

[Документация](README.md) · [Структура файлов](structure.md) · [SDK](sdk.md)

Плагин — manifest и один JavaScript-файл. Player выполняет его в Web Worker,
а интерфейс строит из возвращённых данных. DOM, Vue, Node.js и прямого Tauri API
в контексте плагина нет. Зависимости заранее собираются в `main.js` без внешних
`import` / `require`.

## Создать папку

Из корня `player-plugins` создайте папку `my-plugin/` с двумя файлами.
Для рабочего образца также можно скопировать [examples/hello](../examples/hello/README.md),
затем поменять ID и метаданные.

`my-plugin/plugin.json`:

```json
{
  "id": "author.my-plugin",
  "name": "My plugin",
  "version": "1.0.0",
  "apiVersion": 1,
  "description": "A page inside Player.",
  "author": { "name": "Author", "url": "https://github.com/author" },
  "repository": "https://github.com/author/my-plugin",
  "platforms": ["macos", "windows", "linux", "android"],
  "permissions": ["ui"],
  "contributions": [
    { "id": "home", "kind": "page", "label": "My plugin" }
  ]
}
```

`my-plugin/main.js`:

```js
registerPlugin((ctx) => ({
  activate() {
    ctx.log('My plugin started');
  },
  render({ viewId }) {
    return { type: 'text', text: `Hello from ${viewId}` };
  }
}));
```

Factory синхронно возвращает объект обработчиков. Не делайте её `async`;
асинхронную инициализацию выполняйте в `activate()`.

## Поля plugin.json

| Поле | Назначение и ограничения |
| --- | --- |
| `id` | Постоянный ID до 100 символов, части через точку из `[a-z0-9-]`; `player.*` зарезервирован |
| `name` | Непустое имя до 100 байт UTF-8 |
| `version` | `major.minor.patch`, три неотрицательных целых, без prerelease |
| `apiVersion` | Сейчас только `1` |
| `description` | Описание до 2000 байт UTF-8; необязательно |
| `author` | Имя строкой или `{ "name": "…", "url": "https://…" }`; необязательно |
| `repository`, `homepage` | HTTP(S)-ссылки без встроенных credentials; необязательны |
| `image` | HTTP(S)-ссылка или PNG/JPEG/WebP/GIF data URL до 256 KiB декодированных данных |
| `platforms` | Непустой массив из `macos`, `windows`, `linux`, `android` |
| `permissions` | Только необходимые права из таблицы ниже |
| `contributions` | До 32 вкладов, локальные ID уникальны |

Относительные пути и SVG data URL в `image` не поддерживаются. Для одноцветного
логотипа в меню настроек нужна картинка с прозрачным фоном; карточка сохраняет
исходные цвета. Упакованный JSON целиком должен помещаться в 4 MiB.
Неизвестные поля отклоняются Player.

| Право | Возможности |
| --- | --- |
| `ui` | Страницы, настройки, поиск, действия треков и виджеты |
| `settings` | Собственные настройки с номером ревизии |
| `secrets` | Собственные секреты через credential provider хоста |
| `playback.read` | События состояния воспроизведения |
| `playback.control` | Управление воспроизведением |
| `network.http` | Вызовы `http.request` / `http.download` через хост |
| `library.import`, `library.replace` | Импорт / замена через выданные хостом ресурсы |

`native.discord` и `native.soulseek` доступны встроенным пакетам. `native.android`
используется только при подключении установленного APK: Player формирует binding
с package/class и SHA-256 файла/сертификата. Копирование этого права в обычный JS-пакет
не подключает native-код. См. [Android-пример](../examples/android-native/README.md).

## Упаковать и установить

```sh
pnpm plugins:pack my-plugin /tmp/my-plugin.player-plugin
```

Результат — UTF-8 JSON `{manifest, script}`, не ZIP. Упаковщик не запускает npm,
не собирает зависимости, не встраивает `assets/` и не проверяет все правила Player.
Успешная упаковка ещё не означает, что пакет можно установить или включить.

В Player откройте Settings → Plugins, выберите файл, проверьте manifest и разрешения,
подтвердите установку, затем включите плагин. В боковом меню появится My plugin.
Ошибки entrypoint и выполнения проверяются при включении Worker.

Для установки по ссылке опубликуйте `plugin.json` и `main.js` в одной папке
публичного GitHub-репозитория. Можно указать корень репы, `tree/<ref>/<folder>`,
прямую HTTPS-ссылку на `.player-plugin` или `plugin.json`. Player сохраняет
конкретный GitHub commit и проверяет ID выбранного пакета.

## Обновить

Сохраните тот же ID, увеличьте `version`, отключите установленный плагин и установите
новую версию. Настройки остаются в хосте; локальное состояние Worker создаётся заново.
Для появления в списке плеера отдельно измените [catalog.json](catalog.md).
Встроенные Discord/Soulseek обновляются через закреплённый коммит сборки Player.
