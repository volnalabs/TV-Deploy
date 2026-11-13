<p align="center"><img src="assets/logo-256.png" width="96" alt="TV Deploy"></p>

# TV Deploy

*Простой и современный установщик приложений для Samsung Smart TV.* · English: [README.en.md](README.en.md)

> **Статус: итерация 1 из 4 (pre-alpha).** Работает: поиск и подключение ТВ по SDB, получение DUID, выбор и безопасный анализ `.wgt`, проверка структуры подписи. **Ещё не работает: сертификаты, подписание и установка приложения на ТВ.** Подробности и ограничения — в [docs/KNOWN-LIMITATIONS.md](docs/KNOWN-LIMITATIONS.md). Реальное тестирование на физическом телевизоре **пока не проводилось** — см. [docs/TEST-REPORT.md](docs/TEST-REPORT.md).

TV Deploy — самостоятельный open-source инструмент. Он не является IPTV-плеером, не загружает чужие приложения и не привязан ни к одному конкретному `.wgt`: файл для установки всегда выбираете вы.

## Как это устроено
Интерфейс (React) → Rust-бэкенд (Tauri 2) → встроенный Node.js 22 → официальный Samsung CLI ([tizen-agent-skills](https://github.com/Samsung/tizen-agent-skills), закреплённый коммит, Apache-2.0) → `sdb` → телевизор. См. [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Как включить Developer Mode на Samsung TV
1. Откройте **Apps** на телевизоре.
2. На пульте нажмите **1 2 3 4 5** — появится окно Developer Mode.
3. Переключите **Developer mode** в **On**.
4. В поле **Host PC IP** введите адрес вашего компьютера (TV Deploy показывает его при ошибке подключения).
5. Нажмите OK и **перезагрузите телевизор**.
6. Телевизор и компьютер должны быть в одной локальной сети (не в гостевой Wi-Fi). IP телевизора: *Настройки → Общие → Сеть → Состояние сети*.

## Сборка из исходников
Нужны: Node.js 22, Rust (stable), зависимости Tauri для вашей ОС ([инструкция](https://v2.tauri.app/start/prerequisites/)).
```bash
npm ci
node scripts/fetch-node.mjs linux-x64   # или win-x64 | darwin-arm64 | darwin-x64
npx tauri build
```
Для разработки (`npx tauri dev`) без встроенного Node используется Node.js из PATH — только в debug-сборках.
Тесты: `cargo test --workspace` и `npm test`.

## Требования к Samsung SDK
Пока нужен уже установленный Tizen SDK (папка с `tools/sdb`). Автоматическая установка SDK (около 15 ГБ по оценке самого Samsung CLI) в этой версии **не реализована**.

## Лицензия
Apache-2.0. Встроенный Samsung CLI — © Samsung Electronics, Apache-2.0 (`resources/samsung-cli/LICENSE`, `NOTICE`).
