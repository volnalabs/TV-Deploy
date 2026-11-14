# Test report — EasyTizen 0.9.0

Date: 2026-11-14. Environment of the local runs: Linux x86_64 (Ubuntu 24.04 container), Rust 1.97, Node.js 22.23, OpenSSL 3, xmlsec1, Android SDK platform 36 / NDK 27.2.

This report states what was actually run and what it proves. Anything not listed under "Verified" has not been verified.

## Verified automatically (all passing)

| Suite | Command | Result |
|---|---|---|
| Core (`easytizen-core`) | `EASYTIZEN_REQUIRE_INTEROP=1 cargo test -p easytizen-core --features mock-tv` | 68 tests passed |
| App backend (`easytizen`) | `cargo test -p easytizen` | 18 tests passed |
| Interface and release scripts | `npm test` | 13 tests passed |
| Types | `npm run typecheck` | no errors |
| Lints | `cargo clippy --workspace --all-targets --features easytizen-core/mock-tv -- -D warnings`, `cargo fmt --all -- --check` | clean |

What the important tests establish:

* **SDB client ↔ TV emulator** — handshake, `capability:`, DUID retrieval, a 300 kB byte-exact push, device-side refusal of a bad path, install → launch, cleanup of the uploaded copy, cancellation, recognition of Android ADB endpoints. The emulator is our own implementation of the device side, so this proves internal consistency and correct framing, **not** behaviour of a real TV.
* **WGT signing verified by an independent implementation** — keys and `.p12` files are produced by the OpenSSL CLI with the flags Samsung's CLI uses; both signature documents are then verified by `xmlsec1`, and a tampered file makes verification fail. This proves the XML-DSig output is cryptographically valid.
* **Certificate issuance against an OpenSSL stand-in** — our CSRs are checked (`openssl req -verify`) and signed by OpenSSL acting as the CA; the test asserts the endpoint order, form fields, RSA-2048/SHA-512, and the `URN:tizen:deviceid=` SAN, then signs a package with the issued pair and installs it on the emulator. Keystores written by EasyTizen are read back by OpenSSL. Samsung's real service is **not** contacted.
* **A package signed for one TV is refused by another** (emulated DUID check).
* **WGT safety** — zip-slip names, DTD/entity bombs, oversized XML, non-ZIP input, duplicate entries; the original file is byte-identical after inspection and signing.
* **SDK installer** — progress parsing, streaming of stdout/stderr, failure reporting, and cancellation that stops the installer's background children (process group kill). This test caught a real defect during development.
* **Samsung OAuth callback** — loopback round trip, single decoding of the `code` value, timeout, cancellation, token redaction in debug output.
* **Settings upgrade** from the TV Deploy 0.1 schema and **data-folder migration** from the old identifier.
* **Release tooling** — a release is blocked when an installer is missing, too small or unexpected; every published file name appears in both READMEs, the release notes template and the in-app update check; repository versions agree.

## Verified by building and running locally

| Target | What was done | Result |
|---|---|---|
| Linux `.deb` and `.AppImage` | `npx tauri build --bundles deb,appimage` | built: 48 MB and 118 MB; package `easy-tizen`, depends on `libwebkit2gtk-4.1-0`, `libgtk-3-0` |
| Linux binary | started under Xvfb; window captured (`docs/img/linux-window.png`) | starts, first-run guide rendered, data folder `io.github.volnalabs.easytizen` created |
| Android APK + AAB (arm64 + armv7) | `npx tauri android build --apk --aab --target aarch64 armv7` with a throw-away signing key, as CI does | built: APK 18 MB, AAB 9.7 MB; `apksigner verify` passes; `aapt2` shows package `io.github.volnalabs.easytizen`, version 0.9.0, permissions INTERNET, ACCESS_NETWORK_STATE, ACCESS_WIFI_STATE; no Samsung CLI inside |
| Interface | `scripts/screenshots.mjs` drives the real frontend through the whole flow on desktop and phone viewports with the demo backend | all 17 screens rendered without console errors |

## Not verified

* **Physical Samsung TV:** nothing — connection, DUID, certificate acceptance, installation, launch.
* **Real Android device:** the APK was not installed or run.
* **Real Samsung Account / Samsung CA.**
* **Windows and macOS:** installers are produced by CI only; not run by hand. Code-signing steps in CI have never run with real certificates.
* **Real Tizen SDK installation** through the app (15 GB download) and the SDK engine's signing/installing with the genuine `tz`/`sdb`.

## Tested TV models

None yet. Please report results (model, year, Tizen version, EasyTizen version, engine) in an issue so this table can be filled.

See [ACCEPTANCE-CHECKLIST.md](ACCEPTANCE-CHECKLIST.md) for the hands-on checks and [KNOWN-LIMITATIONS.md](KNOWN-LIMITATIONS.md) for limits.
