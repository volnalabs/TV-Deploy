**EasyTizen {{VERSION}}** — простой способ устанавливать приложения на телевизоры Samsung: с компьютера или прямо с телефона.

## Какой файл скачать

| Устройство | Файл | Как установить |
|---|---|---|
| 📱 **Android** 7.0 и новее (телефон, планшет) | `EasyTizen-Android.apk` | Открыть файл на устройстве и разрешить установку — [инструкция](https://github.com/volnalabs/TV-Deploy#android) |
| 💻 **Windows** 10 и 11 (64-bit) | `EasyTizen-Windows-x64-Setup.exe` | Запустить установщик — [инструкция](https://github.com/volnalabs/TV-Deploy#windows) |
| 💻 **macOS** на процессорах Apple (M1 и новее) | `EasyTizen-macOS-ARM64.dmg` | Открыть и перетащить в «Программы» — [первый запуск](https://github.com/volnalabs/TV-Deploy#macos) |
| 💻 **macOS** на процессорах Intel | `EasyTizen-macOS-Intel.dmg` | То же самое — [первый запуск](https://github.com/volnalabs/TV-Deploy#macos) |
| 🐧 **Linux** (любой дистрибутив, 64-bit) | `EasyTizen-Linux-x64.AppImage` | `chmod +x`, затем запустить — [инструкция](https://github.com/volnalabs/TV-Deploy#linux) |
| 🐧 **Debian, Ubuntu, Mint** (64-bit) | `EasyTizen-Linux-x64.deb` | `sudo apt install ./EasyTizen-Linux-x64.deb` — [инструкция](https://github.com/volnalabs/TV-Deploy#linux) |

> [!TIP]
> **Не знаете, какой Mac?** Меню Apple → «Об этом Mac». Если в строке «Чип» написано Apple — берите `ARM64`, если «Процессор Intel» — `Intel`.

Файлы `Source code` нужны только тем, кто собирает приложение сам. `SHA256SUMS.txt` — контрольные суммы для проверки загрузки.

**Дальше:** [как включить режим разработчика на телевизоре](https://github.com/volnalabs/TV-Deploy#samsung-tv) · [как подключить телевизор, создать сертификат и установить приложение](https://github.com/volnalabs/TV-Deploy#first-install) · [вопросы и ответы](https://github.com/volnalabs/TV-Deploy#faq)

## Что изменилось

{{CHANGES}}

## Подпись установщиков

{{SIGNING}}

## Контрольные суммы SHA-256

{{SHA256_TABLE}}

Проверка: `sha256sum -c SHA256SUMS.txt` (Linux, macOS: `shasum -a 256 -c SHA256SUMS.txt`) или `Get-FileHash <файл> -Algorithm SHA256` (Windows PowerShell).

**[Полная история изменений →](https://github.com/volnalabs/TV-Deploy/blob/main/CHANGELOG.md)** · **[Инструкции и скриншоты →](https://github.com/volnalabs/TV-Deploy#readme)**
