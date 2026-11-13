# Manual acceptance checklist — physical Samsung TV

**Status: NOT YET EXECUTED.** Record the TV model/year, Tizen version, host OS, TV Deploy build and evidence for every run. Do not mark a check complete based only on mock tests.

## Setup and connection

- [ ] Install TV Deploy on Windows 10/11 x64.
- [ ] Install TV Deploy on macOS Apple Silicon.
- [ ] Install TV Deploy on macOS Intel.
- [ ] Install TV Deploy on Ubuntu/Debian x64.
- [ ] First-run setup downloads the official Tizen SDK and Samsung TV Extension, then reports the verified SDK path.
- [ ] Existing SDK is detected and can be selected without modifying the user's existing Tizen Studio installation.
- [ ] Low disk space, missing Bash 4+ on macOS, interrupted download and installer failure produce actionable messages.
- [ ] Find my TV discovers a TV with Developer Mode enabled.
- [ ] Manual IP connection works.
- [ ] Connected is shown only after Samsung SDB confirms the connection.
- [ ] DUID returned by Samsung tooling is displayed and remains associated with the selected TV.
- [ ] Saved devices survive restart but are not shown as connected until revalidated.
- [ ] Wrong IP, closed port, disabled Developer Mode and incorrect Host PC IP are diagnosed.

## Package validation

- [ ] A valid unsigned WGT is accepted and its name, package ID, version and size are shown.
- [ ] A valid signed WGT is accepted and described only as structurally signed until trusted verification is performed.
- [ ] Empty, non-ZIP, wrong-extension and malformed config.xml files are rejected.
- [ ] Zip-slip entries, excessive entries, oversized XML and suspicious compression ratios are rejected.
- [ ] Original WGT remains byte-for-byte unchanged throughout signing and installation.

## Certificates and installation

- [ ] Samsung Account OAuth opens in the system browser and returns to TV Deploy.
- [ ] Author Certificate is created by the official Samsung certificate manager.
- [ ] Distributor Certificate contains the selected TV DUID.
- [ ] Signing profile is created and selected for the same TV.
- [ ] A second TV cannot accidentally use a DUID-bound profile from the first TV.
- [ ] A signed working copy is produced by the Tizen SDK; no original WGT is overwritten.
- [ ] Samsung install command succeeds and the TV confirms the application is installed.
- [ ] Optional launch-after-install starts the app.
- [ ] Invalid certificate, expired/failed Samsung authorization and install failure show a useful error.

## UI and resilience

- [ ] File picker and drag-and-drop both work.
- [ ] Russian/English language switching works.
- [ ] Light/dark/system themes work.
- [ ] Cancel during scan/connect does not leave a stuck process.
- [ ] Relaunching TV Deploy does not falsely restore a stale connected state.

## Release readiness

- [ ] CI passes on Windows, macOS and Linux.
- [ ] The four platform build jobs produce the expected installer artifacts.
- [ ] Windows and macOS signing/notarization status is clearly shown.
- [ ] README instructions match the packaged application.
