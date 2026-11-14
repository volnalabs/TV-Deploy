# Architecture

```
React/TS interface ──invoke(typed)──▶ src-tauri (Rust) ──▶ easytizen-core
 (no process, no fs access)           validates every argument, owns state
                                            │
                  ┌─────────────────────────┴─────────────────────────────┐
           SDK engine (desktop)                              Built-in engine (all platforms)
     sdk_engine.rs + sdk_installer.rs                               builtin.rs
     bundled Node.js → pinned Samsung CLI                 core::sdb · core::sign · core::samsung_ca
     → Tizen SDK (sdb, tz, cert-manager) → TV                          → TV
```

## Crates and folders

| Path | Role |
|---|---|
| `crates/easytizen-core` | No UI dependency. `package` (safe WGT inspection), `validate`, `diagnostics`, `connectivity`, `samsung/*` (typed invocations of the Samsung CLI and envelope parsing), `sdb/*` (SDB protocol client, LAN scan, install helpers, TV emulator), `sign/*` (PKCS#12 identities, XML-DSig WGT signer), `samsung_ca/*` (Samsung Account OAuth callback, CSR, CA client), `update`. |
| `src-tauri` | Tauri 2 shell. `api.rs` — the only command surface, with validation and engine dispatch; `sdk_engine.rs`, `sdk_installer.rs`, `toolchain.rs` — desktop engine; `builtin.rs` — native engine; `settings.rs`, `history.rs`, `migrate.rs` — state. |
| `src-tauri/gen/android` | Generated and then adjusted Android project (permissions, target SDK, release signing). Committed. |
| `src` | React interface. `services/api.ts` is the typed bridge; `services/demo.ts` is an in-memory backend used only in a plain browser. |
| `resources/samsung-cli` | Samsung `tizen-sdk-skills` `common/lib`, installer scripts and TV CA certificates, pinned (`PINNED.json`). Desktop bundles only; the CA certificates are also compiled into the core. |

## The two engines

**SDK engine** (default on desktop). EasyTizen orchestrates Samsung's own tools: the pinned Node.js CLI for scan/connect/DUID/certificates/install and `tz pack` for signing. Every script runs with a private `HOME` (`<app data>/samsung-home`) so a user's Tizen Studio configuration is never touched. `TIZEN_SDK_PATH` is ignored by the scripts; the SDK path is written by `sdk-init-cli.js`.

**Built-in engine** (Android always; optional on desktop). Implemented in Rust:

* `sdb::client` — one TCP connection, one stream at a time: `CNXN` handshake (`host::`, version `0x02000000`), `capability:`, `shell:<cmd>`, `sync:` push (`SEND`/`DATA`/`DONE`). Deadlines and cancellation on every read. Android ADB endpoints are recognised and rejected.
* Device commands mirror Samsung's tools (table in `sdb/mod.rs`): `0 getduid`, `0 vd_appinstall <app-id> <path>`, `0 was_execute`, `0 vd_appuninstall`, `0 rmfile`. Only validated words are ever spliced into a device command.
* `sign::wgt` — `author-signature.xml` and `signature1.xml`, RSA-SHA512 with SHA-512 digests, generated directly in canonical form. The archive is rewritten by raw-copying original entries; the user's file is opened read-only.
* `samsung_ca` — the flow of Samsung's open-source CLI (`samsung-auth.js`, `samsung-api.js`, `samsung-cert.js`) ported to Rust: loopback OAuth callback on 127.0.0.1:4794–4813, RSA-2048 keys and SHA-512 CSRs generated locally, multipart requests to `svdca.samsungqbe.com`, PKCS#12 packaging with the Samsung VD CA. TLS certificates are verified (the reference implementation disables verification).

A certificate profile belongs to the engine that created it (`engine` field in settings).

## Security properties

* The frontend has no shell, filesystem or process permissions (`capabilities/default.json`): window events, progress events and the native pickers only. CSP forbids remote content.
* Arguments are validated in `api.rs` and again in the core; the Samsung CLI is started with an argv vector from an allow-list of scripts, never through a shell.
* WGT inspection reads the central directory, `config.xml` and signature XML into memory with hard caps (512 MB archive, 20 000 entries, 2 GB uncompressed, ratio guard, 1 MB XML) and rejects zip-slip names and DTDs. Nothing from a package is extracted to disk or executed.
* Secrets: certificate passwords travel through an environment variable to the Samsung CLI (never argv) or stay in memory for the session (built-in engine). The Samsung access token lives in memory only. Private keys are written only inside password-protected `.p12` files (0600 on Unix). Error details and the diagnostics report pass through a redactor.
* Opening URLs is limited to an allow-list (this repository on GitHub, Samsung account pages).

## Data migration

The data folder is derived from the bundle identifier. `migrate.rs` copies `settings.json` and `samsung-home` from `io.github.volnalabs.tvdeploy` on first start; `settings.rs` upgrades the 0.1 schema (single `duid` → `duids`). The old folder is left in place.
