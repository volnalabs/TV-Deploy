# Direct dependencies

Exact versions are locked in `Cargo.lock` and `package-lock.json`; full trees: `cargo tree`, `npm ls`.

## Rust

### easytizen-core
| Crate | Purpose | License |
|---|---|---|
| serde, serde_json | data shapes, Samsung CLI envelopes | MIT / Apache-2.0 |
| thiserror | error types | MIT / Apache-2.0 |
| zip (deflate) | reading and rewriting `.wgt` archives | MIT |
| roxmltree | `config.xml` and signature parsing (no DTD support by design) | MIT / Apache-2.0 |
| base64 | signature and certificate encoding | MIT / Apache-2.0 |
| rsa, sha2 | RSA-2048 keys, RSA-SHA512 signatures, digests | MIT / Apache-2.0 |
| x509-cert | CSR building, certificate parsing | MIT / Apache-2.0 |
| p12-keystore | reading and writing PKCS#12 keystores | MIT / Apache-2.0 |
| getrandom | system randomness for key generation | MIT / Apache-2.0 |
| ureq (rustls, webpki-roots) | HTTPS to Samsung's CA and GitHub's API | MIT / Apache-2.0 |
| tempfile (dev) | tests | MIT / Apache-2.0 |

### easytizen (src-tauri)
| Crate | Purpose | License |
|---|---|---|
| tauri, tauri-build | application shell | MIT / Apache-2.0 |
| tauri-plugin-dialog | native open/save pickers | MIT / Apache-2.0 |
| tauri-plugin-fs | reading Android `content://` documents from Rust | MIT / Apache-2.0 |
| tauri-plugin-opener | opening the Samsung sign-in page and GitHub links | MIT / Apache-2.0 |
| fs2 | free disk space | MIT / Apache-2.0 |
| serde, serde_json | settings, history | MIT / Apache-2.0 |

## npm

| Package | Purpose | License |
|---|---|---|
| @tauri-apps/api, @tauri-apps/plugin-dialog | bridge to the backend, pickers | MIT / Apache-2.0 |
| react, react-dom | interface | MIT |
| lucide-react | interface icons | ISC |
| tailwindcss, @tailwindcss/vite (dev) | styling | MIT |
| vite, @vitejs/plugin-react, typescript, vitest, @tauri-apps/cli (dev) | build and tests | MIT / Apache-2.0 |

## Vendored and fetched

| Component | Where | License |
|---|---|---|
| Samsung `tizen-sdk-skills` `common/lib`, installer scripts, Samsung VD CA certificates @ `1509f7b9…` (v1.7.0) | `resources/samsung-cli`, pinned in `PINNED.json`; desktop bundles (CA certificates also in the core) | Apache-2.0 — `LICENSE`, `NOTICE` shipped |
| Node.js 22.23.4 | fetched at build time by `scripts/fetch-node.mjs` (SHA-256 checked), desktop bundles only | MIT |
| Tizen SDK, Samsung TV Extension | **not distributed**; downloaded on the user's machine by Samsung's installer | Samsung's terms |
