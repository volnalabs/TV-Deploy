<p align="center"><img src="assets/logo-256.png" width="96" alt="TV Deploy"></p>

# TV Deploy

*Простой и современный установщик приложений для Samsung Smart TV.* · English: [README.en.md](README.en.md)

> **Статус: Public Preview 0.1.0.** В приложении реализован основной сценарий: подготовка Tizen SDK и Samsung TV Extension, поиск и подключение телевизора, получение DUID, создание Samsung Author/Distributor certificates и signing profile, проверка пользовательского WGT, подписание рабочей копии и установка на выбранный TV. **Физический телевизор и готовые установщики Windows/macOS/Linux ещё не прошли acceptance-проверку.** Это preview, а не подтверждённый production-релиз 1.0.

TV Deploy — самостоятельный open-source инструмент. Он не является IPTV-плеером, не загружает сторонние приложения и не привязан к конкретному WGT: файл для установки всегда выбирает сам пользователь.

## Основной сценарий

1. Подготовьте инструменты Samsung прямо в TV Deploy либо выберите уже установленный SDK.
2. Включите Developer Mode на телевизоре, найдите его в локальной сети и подключитесь.
3. Выберите собственный файл `.wgt` на компьютере.
4. Создайте Samsung certificate profile для DUID этого телевизора или выберите существующий.
5. TV Deploy создаст подписанную копию пакета и передаст её на выбранный телевизор.

Оригинальный WGT не перезаписывается. Подписание выполняется средствами Tizen SDK; установка — официальным Samsung CLI.

## Возможности

- Tizen SDK и Samsung TV Extension: обнаружение существующей установки или запуск официальных installer scripts.
- Samsung SDB: сканирование локальной сети, ручное подключение, повторное подключение сохранённых телевизоров.
- DUID: получение через Samsung certificate CLI после подтверждённого SDB-подключения.
- Сертификаты: Samsung Account OAuth в системном браузере, создание Author Certificate, Distributor Certificate для DUID и signing profile.
- WGT: проверка ZIP-структуры и `config.xml`, извлечение метаданных, проверка наличия signature XML.
- Подписание и установка: отдельная копия WGT, Tizen `tz pack` и Samsung `project-manager-cli install`.
- Русский и английский языки; светлая, тёмная и системная темы.

## Ограничения preview

- Проверка подписи в карточке файла структурная: она не является независимой криптографической проверкой действительности цепочки сертификатов.
- Samsung Distributor Certificate привязан к DUID телевизора; профиль для одного TV нельзя считать универсальным.
- На macOS SDK installer требует Bash 4+; штатный `/bin/bash` может быть старее. TV Deploy объясняет эту зависимость, но не устанавливает Bash самостоятельно.
- Импорт существующих сертификатов через UI, история установок и автоматическое обновление самого TV Deploy пока не реализованы.
- Сборки пока не подписываются сертификатом Windows/macOS и не проходят macOS notarization.
- Реальная установка должна быть проверена на физическом Samsung TV перед production-релизом.

Подробнее: [ограничения](docs/KNOWN-LIMITATIONS.md), [отчёт тестирования](docs/TEST-REPORT.md), [ручной acceptance checklist](docs/ACCEPTANCE-CHECKLIST.md).

## Как включить Developer Mode на Samsung TV

1. Откройте **Apps** на телевизоре.
2. На пульте нажмите **1 2 3 4 5** — появится окно Developer Mode.
3. Включите **Developer Mode**.
4. В поле **Host PC IP** введите IP-адрес компьютера, на котором запущен TV Deploy.
5. Нажмите OK и перезагрузите телевизор.
6. Телевизор и компьютер должны быть доступны в одной локальной сети. IP телевизора можно посмотреть в настройках сети телевизора.

Названия пунктов меню могут отличаться в зависимости от модели и версии Tizen.

## Сборка из исходников

Нужны Node.js 22, Rust stable и системные зависимости Tauri для вашей ОС: [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).

```bash
npm ci
node scripts/fetch-node.mjs linux-x64   # win-x64 | darwin-arm64 | darwin-x64
npx tauri build
```

Для разработки (`npx tauri dev`) в debug-режиме допускается Node.js из PATH. В release-сборки включается отдельный Node runtime, скачиваемый из официального источника и сверяемый по SHA-256.

## CI и релизы

GitHub Actions проверяет Rust и frontend, а Build workflow собирает Windows x64, macOS ARM64/Intel и Linux x64. Тег `v*` запускает матричную сборку и создаёт GitHub Release только после успешной сборки всех платформ. Опубликованные установщики пока unsigned.

## Лицензия

Apache-2.0. Встроенные Samsung CLI и скрипты распространяются согласно лицензии и NOTICE Samsung; см. `resources/samsung-cli/LICENSE` и `resources/samsung-cli/NOTICE`.
