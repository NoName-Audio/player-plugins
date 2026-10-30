# Android APK / Rust example

Эксперимент самостоятельного native-плагина. UI работает в стандартном Worker,
APK содержит Kotlin entrypoint и Rust JNI-библиотеку. APK не добавляется в сборку
Player. Контракт и ограничения: [android-native scope](https://github.com/verncat/player/blob/feat/plugins/docs/plugins/scope/android-native-scope.md).

## Файлы и путь вызова

| Файл | Назначение |
| --- | --- |
| [plugin.json](plugin.json) | Метаданные, `native.android` и страница `home` |
| [main.js](main.js) | Дерево UI и `ctx.android.call('echo', {message})` |
| [src/main/AndroidManifest.xml](src/main/AndroidManifest.xml) | Discovery marker, plugin ID, API version и имя entry class |
| [NativePlugin.kt](src/main/java/dev/verncat/player/example/nativeplugin/NativePlugin.kt) | Reflection ABI, загрузка `.so`, JSON-запросы через JNI |
| [native/src/lib.rs](native/src/lib.rs) | JNI entrypoint, метод `echo`, счётчик вызовов и unit test |
| [native/Cargo.toml](native/Cargo.toml) / [Cargo.lock](native/Cargo.lock) | Rust cdylib и закреплённые зависимости |
| [build.gradle.kts](build.gradle.kts) | Генерация JSON assets, запуск сборки Rust и упаковка APK |
| [settings.gradle.kts](settings.gradle.kts) | Android/Kotlin Gradle plugins и repositories |
| [gradle.properties](gradle.properties) / [proguard-rules.pro](proguard-rules.pro) | Параметры сборки и сохранение entrypoint |

Путь вызова: Worker `main.js` → gateway Player → Kotlin `NativePlugin.call()` →
JNI `nativeCall()` → Rust `echo`. APK исполняется внутри процесса Player; native-код
не изолирован Worker. Строки/результаты передаются через JSON.

Gradle формирует `build/generated/pluginAssets/player-plugin.json` из manifest и JS.
Binding установленного APK с SHA-256 файла и сертификата формирует загрузчик Player:
в исходный `plugin.json` эти значения вручную не добавляются.

## Сборка

Нужны Android SDK/NDK, JDK 17+, Rust Android target и Node.js. Из корня player-plugins:

```sh
export ANDROID_HOME="$HOME/Library/Android/sdk" # путь вашего SDK
rustup target add aarch64-linux-android
pnpm plugins:android:build
```

Результат: `examples/android-native/build/outputs/apk/debug/android-native-debug.apk`.
Debug APK подписан локальным debug key. Для публикации настройте release signing
собственным постоянным ключом; `.player-plugin` packer здесь не устанавливает APK.
Cargo.lock включён, Rust собирается через NDK clang без зависимости от cargo-ndk.

Другие ABI можно выбрать свойством Gradle; сначала установите их Rust targets:

```sh
./gradlew -p examples/android-native assembleDebug -PpluginAbis=arm64-v8a,x86_64
```

## Подключение

В репозитории **Player** соберите Android-приложение через `pnpm android:build`. Для native
Opus-зависимости Player задайте `ANDROID_NDK_HOME` на каталог установленной
версии NDK внутри `$ANDROID_HOME/ndk`. Установите
Player и APK-пример через системный installer или adb, затем:

1. Settings → Plugins → Find APK plugins.
2. Native example → Review connection; проверьте сведения и разрешение native-кода.
3. Install and grant these permissions; включите плагин.
4. На странице Native example должен появиться `Rust via JNI` и архитектура.
5. Введите сообщение и нажмите Call Rust; счётчик вызовов увеличится.

При обновлении APK отключите подключение, пересканируйте и подтвердите новую
версию. Если `.so` уже загружалась, полностью перезапустите процесс Player.
Удаление подключения в Player не удаляет установленный Android APK.

## Проверка

```sh
cargo test --manifest-path examples/android-native/native/Cargo.toml
pnpm test
adb -s emulator-5554 install -r examples/android-native/build/outputs/apk/debug/android-native-debug.apk
```

Интеграционный тест загрузчика остаётся в репозитории Player.
После сборки и установки актуального Player и APK-примера, из его `src-tauri/gen/android`:

```sh
./gradlew :app:assembleArm64DebugAndroidTest -x :app:rustBuildArm64Debug
adb -s emulator-5554 install -r app/build/outputs/apk/androidTest/arm64/debug/app-arm64-debug-androidTest.apk
adb -s emulator-5554 shell am instrument -w -e class dev.verncat.player.ApkPluginsTest dev.verncat.player.test/androidx.test.runner.AndroidJUnitRunner
```

Исключение rustBuild в этой команде допустимо только после нормальной сборки
Player: Gradle иначе требует запущенного Tauri CLI build server.
Тест вызывает настоящий Kotlin-загрузчик и Rust из установленного APK.

[Документация](../../docs/README.md) · [Структура файлов](../../docs/structure.md) · [Инструменты](../../docs/tooling.md)
