# Каталог расширений

[Документация](README.md) · [Файл catalog.json](../catalog.json)

Player получает список из
`https://raw.githubusercontent.com/verncat/player-plugins/main/catalog.json`.
Репозиторий должен быть публичным. Загрузка каталога не скачивает исполняемый код:
пакет загружается отдельно после выбора Install.

## Формат записи

```json
{
  "schemaVersion": 1,
  "updatedAt": "2026-10-31",
  "plugins": [
    {
      "id": "example.hello",
      "name": "Hello Player",
      "description": "An example extension with a page, settings and playback events.",
      "repository": "https://github.com/verncat/player-plugins",
      "installUrl": "https://github.com/verncat/player-plugins/tree/main/examples/hello",
      "addedAt": "2026-10-27",
      "updatedAt": "2026-10-31",
      "version": "1.0.0",
      "author": "Player contributors",
      "platforms": ["macos", "windows", "linux", "android"],
      "tags": ["example", "playback", "settings"]
    }
  ]
}
```

| Поле | Правило |
| --- | --- |
| `schemaVersion` | Версия схемы всего файла, сейчас `1` |
| `updatedAt` каталога | Календарная дата `YYYY-MM-DD` |
| `plugins` | Не более 256 записей с уникальными ID |
| `id`, `name`, `description`, `repository`, `addedAt`, `platforms` | Обязательные поля записи |
| `installUrl` | HTTPS-ссылка на папку `plugin.json` + `main.js` или готовый `.player-plugin`; без неё используется `repository` |
| `version`, `author`, `image`, `updatedAt`, `tags` | Необязательные метаданные записи |
| `tags` | До 12 непустых тегов, каждый до 40 байт UTF-8 |

`id`, `name`, платформы, автор, изображение и версия подчиняются правилам
[manifest](development.md#поля-pluginjson). Ссылки repository/installUrl — HTTPS
без credentials. `addedAt` не позже даты каталога; дата обновления записи — между
её добавлением и датой каталога. Размер всего ответа — не более 512 KiB.
Неизвестные поля и невалидная запись приводят к отклонению всего ответа.

## Добавить или обновить запись

1. Подготовьте существующий пакет с постоянным ID и опубликованными `plugin.json` / `main.js`.
2. Укажите installUrl выбранной папки; ссылка на корень monorepo не найдёт вложенный пакет.
3. Согласуйте ID, имя, версию и платформы с manifest. При обновлении сохраняйте `addedAt`.
4. Измените `updatedAt` записи и каталога, запустите `pnpm test` и `pnpm test:plugin-sdk`.
5. Опубликуйте изменения в `main`; в Player нажмите Refresh catalog.

Текущий `tests/packages.test.mjs` проверяет записи из этой репы и известные папки
пакетов. При добавлении нового локального плагина дополните массив `folders` теста.
Плеер поддерживает каталожные ссылки на другие публичные репозитории, но локальная
проверка этой репы пока рассчитана на собственные пакеты.

Внешние пакеты с ID `player.*` и правами `native.discord` / `native.soulseek`
отклоняются: Discord/Soulseek уже поставляются в сборке Player. Android-пример
подключается через установленный APK и Find APK plugins, без записи в этом списке.

## Получение и установка

Список обновляется при открытии раздела и по Refresh catalog. Встроенной копии,
дискового кеша и offline fallback нет. При ошибке прежний список очищается,
показывается toast; установка из файла или по прямой ссылке остаётся доступной.

После Install Player проверяет реальный manifest и совпадение ID, затем показывает
разрешения и fingerprint. Каталог не выдаёт права, не подтверждает автора и не
обновляет уже установленный код автоматически. Обновление встроенных пакетов
через submodule — отдельная операция: [поставка](tooling.md#связь-с-player).
