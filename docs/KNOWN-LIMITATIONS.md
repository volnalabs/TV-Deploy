# Known limitations — Public Preview 0.1.0

This document distinguishes code that is implemented from behavior that has been verified on actual hardware.

## Implemented in the current source

- Official Samsung/Tizen SDK and Samsung TV Extension installer orchestration (Windows PowerShell; Bash 4+ on macOS/Linux).
- Existing SDK discovery and SDK path initialization in TV Deploy's private HOME.
- Samsung CLI-based LAN scan, SDB connect/disconnect and DUID retrieval.
- Read-only WGT inspection with archive/path/size guards and config.xml metadata extraction.
- Samsung Author Certificate and DUID-bound Distributor Certificate creation through the pinned Samsung CLI, using Samsung Account OAuth in the system browser.
- Signing-profile creation and WGT signing to a separate working copy through the Tizen Core `tz pack` command.
- Installation of the signed WGT through Samsung's `project-manager-cli install` command, with optional launch after installation.
- Local settings, saved devices, Russian/English strings and light/dark/system themes.

## Not yet verified / not implemented

1. **Physical Samsung TV acceptance has not been completed.** The automated end-to-end test drives the genuine Samsung scripts against a mock SDB executable. That checks argv and JSON handling, not a real TV, actual Samsung Account authorization or the online certificate authority.
2. **Native desktop visual QA is pending.** Drag-and-drop, layout, theme changes and dialogs still need a visual pass in a packaged app on each OS.
3. **Cross-platform packaging must be confirmed by PR CI.** The build matrix is configured for Windows x64, macOS ARM64, macOS Intel and Linux x64; do not claim an artifact exists until the corresponding job succeeds.
4. **macOS requires Bash 4+.** The built-in /bin/bash on macOS is often 3.2, while the Samsung installer requires a newer Bash. The current app reports this requirement; it does not bootstrap Bash or Homebrew.
5. **Signature status before signing is structural only.** Presence and XML shape of signature files are checked; this is not a cryptographic trust-chain or DUID verification. The official Tizen signing command is used for signing, and the Samsung CLI response is checked before reporting installation success.
6. **Certificate import UI is not implemented.** Users can create a new Samsung online-CA author/distributor pair and profile; migrating an existing certificate from a separate Tizen Studio installation requires a manual workflow.
7. **No installation history yet.** The app does not currently keep a dedicated, searchable installation history.
8. **No app self-updater.** Updates to TV Deploy are currently delivered through GitHub Releases.
9. **Artifacts are unsigned.** Windows code signing, macOS Developer ID signing and notarization are not configured until signing secrets are provided.
10. **Node checksum authenticity.** The bundled Node runtime archive is checked against the SHA-256 value in the HTTPS-hosted SHASUMS file; GPG verification of that checksum manifest is not implemented yet.
11. **SDK setup progress/cancellation.** SDK installer output is captured and surfaced on failure, but granular live progress and cancellation of a long-running vendor installer are not yet provided.
12. **Supported package type.** The UI intentionally accepts `.wgt` only. Other Tizen package extensions are not claimed as Samsung TV-compatible.

## Release gate

Do not publish a production `1.0.0` until the manual checklist in [ACCEPTANCE-CHECKLIST.md](ACCEPTANCE-CHECKLIST.md) is completed on at least one physical Samsung TV and the platform build jobs have produced valid installers.
