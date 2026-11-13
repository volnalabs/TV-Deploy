<p align="center"><img src="assets/logo-256.png" width="96" alt="TV Deploy"></p>

# TV Deploy

*A simple and modern application installer for Samsung Smart TVs.* · Русский: [README.md](README.md)

> **Status: iteration 1 of 4 (pre-alpha).** Working: TV discovery and SDB connection, DUID retrieval, choosing and safely analysing a `.wgt`, signature-structure check. **Not working yet: certificates, signing and installing an app on the TV.** See [docs/KNOWN-LIMITATIONS.md](docs/KNOWN-LIMITATIONS.md). **No test on a physical TV has been performed yet** — see [docs/TEST-REPORT.md](docs/TEST-REPORT.md).

TV Deploy is a standalone open-source tool. It is not an IPTV player, never downloads third-party apps and is not tied to any specific `.wgt`: you always choose the file to install.

## How it works
React UI → Rust backend (Tauri 2) → bundled Node.js 22 → official Samsung CLI ([tizen-agent-skills](https://github.com/Samsung/tizen-agent-skills), pinned commit, Apache-2.0) → `sdb` → TV. See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Enable Developer Mode on a Samsung TV
1. Open **Apps** on the TV. 2. Press **1 2 3 4 5** on the remote. 3. Turn **Developer mode** On. 4. Enter your computer's address in **Host PC IP**. 5. Press OK and **restart the TV**. 6. The TV and computer must be on the same local network.

## Build from source
Node.js 22, Rust (stable), [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/):
```bash
npm ci
node scripts/fetch-node.mjs linux-x64   # or win-x64 | darwin-arm64 | darwin-x64
npx tauri build
```
Tests: `cargo test --workspace`, `npm test`.

## Samsung SDK
An existing Tizen SDK (folder containing `tools/sdb`) is required for now. Automatic SDK installation (~15 GB according to the Samsung CLI itself) is **not implemented** yet.

## License
Apache-2.0. Bundled Samsung CLI © Samsung Electronics, Apache-2.0.
