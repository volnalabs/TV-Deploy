<p align="center"><img src="assets/logo-256.png" width="96" alt="TV Deploy"></p>

# TV Deploy

*A simple and modern application installer for Samsung Smart TVs.* · Русский: [README.md](README.md)

> **Status: Public Preview 0.1.0.** The main workflow is implemented: prepare the Tizen SDK and Samsung TV Extension, discover/connect to a TV, retrieve its DUID, create Samsung Author/Distributor certificates and a signing profile, inspect a user-selected WGT, sign a working copy and install it on the selected TV. **A physical TV and the Windows/macOS/Linux installers have not completed acceptance testing.** This preview is not a verified 1.0 production release.

TV Deploy is a standalone open-source utility. It is not an IPTV player, does not download third-party apps, and is not tied to a specific WGT: the user always selects the package to install.

## Main workflow

1. Prepare Samsung tools inside TV Deploy, or select an existing SDK.
2. Enable Developer Mode, discover the TV on the local network and connect.
3. Choose a local `.wgt` file.
4. Create a Samsung certificate profile for that TV's DUID, or select an existing profile.
5. TV Deploy signs a working copy of the package and sends it to the selected TV.

The original WGT is not overwritten. Signing is performed by the Tizen SDK; installation is performed by the official Samsung CLI.

## Features

- Tizen SDK and Samsung TV Extension: detect an existing installation or run official installer scripts.
- Samsung SDB: LAN discovery, manual connection and reconnecting saved TVs.
- DUID retrieval through Samsung's certificate CLI after SDB confirms the connection.
- Certificates: Samsung Account OAuth in the system browser, Author Certificate, DUID-bound Distributor Certificate and signing profile creation.
- WGT inspection: safe ZIP structure and `config.xml` validation, metadata extraction and signature XML presence checks.
- Signing and installation: a separate WGT copy, Tizen `tz pack` and Samsung `project-manager-cli install`.
- Russian and English UI; light, dark and system themes.

## Preview limitations

- The signature indicator in the package details is structural. It is not an independent cryptographic validation of the certificate chain.
- A Samsung Distributor Certificate is associated with the TV DUID; a profile for one TV must not be treated as universal.
- On macOS the SDK installer requires Bash 4+; the system `/bin/bash` may be older. TV Deploy explains the dependency but does not install Bash itself.
- Importing existing certificates in the UI, installation history and automatic TV Deploy updates are not implemented yet.
- Windows/macOS code signing and notarization are not configured. Build artifacts are unsigned.
- The complete workflow must be tested on a physical Samsung TV before a production release.

See [known limitations](docs/KNOWN-LIMITATIONS.md), [test report](docs/TEST-REPORT.md) and the [manual acceptance checklist](docs/ACCEPTANCE-CHECKLIST.md).

## Enable Developer Mode on a Samsung TV

1. Open **Apps** on the TV.
2. Press **1 2 3 4 5** on the remote to open Developer Mode.
3. Turn **Developer Mode** on.
4. Enter the computer's IP address in **Host PC IP**.
5. Confirm and restart the TV.
6. The TV and computer must be reachable on the same local network. The TV's IP is available in its network settings.

Menu names may differ by TV model and Tizen version.

## Build from source

You need Node.js 22, stable Rust and the system dependencies listed in the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).

```bash
npm ci
node scripts/fetch-node.mjs linux-x64   # win-x64 | darwin-arm64 | darwin-x64
npx tauri build
```

Debug builds may use Node.js from PATH. Release builds bundle a separate Node runtime downloaded from the official source and checked against its SHA-256 checksum.

## CI and releases

GitHub Actions checks Rust and the frontend and builds Windows x64, macOS ARM64/Intel and Linux x64. A `v*` tag starts the full build matrix and creates a GitHub Release only after every platform build succeeds. Installers are currently unsigned.

## License

Apache-2.0. Bundled Samsung CLI scripts are redistributed under Samsung's license and NOTICE; see `resources/samsung-cli/LICENSE` and `resources/samsung-cli/NOTICE`.
