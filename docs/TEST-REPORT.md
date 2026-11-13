# Test report — iteration 1

Environment: Ubuntu 24.04 sandbox, Rust 1.91.1, Node 22.23.4. **No display, no Samsung TV, no Windows/macOS.**

## Executed and passing
| Suite | Result | What it covers |
|---|---|---|
| `cargo test --workspace` | 38 passed (36 core + 2 settings) | IP/port/subnet/serial/path validation incl. injection attempts; WGT analysis (metadata, signature states, zip-slip names, DTD/entity bomb, oversized XML, non-zip, empty, wrong extension, original file untouched); envelope parsing against **real captured output** of the Samsung scripts (incl. noise on stdout and the legacy `error` shape); diagnosis mapping; redaction; TCP probe (open/refused on loopback); process runner (capture, timeout, cancel, crash without envelope, missing script/node); settings round-trip + no secret-like fields |
| e2e `init_connect_duid_through_real_scripts` | passed | **Real vendored Samsung scripts** (`sdk-init`, `remote-device connect`, `cert-manager acquire-duid`) driven by our runner against a **mock `sdb` shell script**. Confirms private-HOME mechanism, argv construction, JSON shapes. Does **not** test a TV |
| `npm test` (vitest) | 6 passed | ru/en key parity and placeholder parity, language resolution, formatting, IPv4 shape |
| `tsc --noEmit`, `vite build`, `cargo check` (Tauri crate), `cargo clippy -D warnings` (core), `cargo fmt` | clean | static checks |

A bug found by the e2e test: the app's root `package.json` (`"type": "module"`) made Node treat the vendored CommonJS
Samsung scripts as ES modules. Fixed with `resources/samsung-cli/package.json` (`"type": "commonjs"`).

## NOT executed (do not assume these work)
* Any run on a physical Samsung TV (connect, DUID, Developer-Mode/Host-IP diagnoses) — see `ACCEPTANCE-CHECKLIST.md`.
* Launching the app window / visual check / drag-and-drop / dark theme.
* Windows, macOS (ARM64/Intel) builds; `tauri build` bundling; `scripts/fetch-node.mjs`; GitHub Actions workflows.
* `cargo clippy`/`fmt` on the Tauri crate in CI form (core only was linted).
