# Releasing EasyTizen

## Cutting a release

1. Set the new version in **three** places (they must match): `package.json`, `Cargo.toml` (`[workspace.package]`) and `src-tauri/tauri.conf.json`. Run `cargo check` so `Cargo.lock` follows, and `npm install --package-lock-only`.
2. Add a `## [X.Y.Z] — date` section to `CHANGELOG.md`. It becomes the "What changed" part of the release page.
3. Merge to `main` and wait for CI to pass.
4. GitHub → **Actions → Release → Run workflow**, enter `X.Y.Z`.

The workflow (`.github/workflows/release.yml`):

1. refuses to start if the version does not match the repository, the tag `vX.Y.Z` already exists, or the changelog section is missing;
2. runs all checks (`checks.yml`): Rust core on Linux/macOS/Windows, frontend, app backend;
3. builds every installer (`build.yml`): Windows x64, macOS ARM64, macOS Intel, Linux AppImage + DEB, Android APK (+ AAB as a workflow artifact);
4. `scripts/release.mjs verify` — all six installers present, plausibly sized, nothing unexpected;
5. computes SHA-256 (`SHA256SUMS.txt`) and renders the release notes from `.github/release-notes.md`;
6. creates the tag and the GitHub Release and uploads the files.

If any check or any platform build fails, step 6 never runs: **no partial release is published.**

Published file names (do not rename — README links, the update check and release notes depend on them; `scripts/release.test.ts` enforces this):

```
EasyTizen-Android.apk
EasyTizen-Windows-x64-Setup.exe
EasyTizen-macOS-ARM64.dmg
EasyTizen-macOS-Intel.dmg
EasyTizen-Linux-x64.AppImage
EasyTizen-Linux-x64.deb
SHA256SUMS.txt
```

Permanent links: `https://github.com/volnalabs/TV-Deploy/releases/latest/download/<file>`.

The Android App Bundle (`EasyTizen-Android-GooglePlay.aab`) is deliberately not attached to the release — it cannot be installed by hand. Download it from the workflow run's artifacts (`google-play-bundle`) when uploading to Google Play.

## Code signing

Signing happens only when the secrets below exist (Settings → Secrets and variables → Actions). Without them nothing is faked; the release notes state the actual status of each platform.

### Android

| Secret | Value |
|---|---|
| `ANDROID_KEYSTORE_BASE64` | `base64 -w0 easytizen.jks` |
| `ANDROID_KEYSTORE_PASSWORD` | keystore password (also used as the key password) |
| `ANDROID_KEY_ALIAS` | key alias |

Create the key once and keep it safe — losing it means users must uninstall before updating:

```sh
keytool -genkeypair -v -keystore easytizen.jks -alias easytizen -keyalg RSA -keysize 4096 -validity 10000
```

Without these secrets the APK is signed with a throw-away key generated for that build (Android refuses unsigned APKs). Such builds cannot update one another.

### Windows

| Secret | Value |
|---|---|
| `WINDOWS_CERTIFICATE` | base64 of the code-signing `.pfx` |
| `WINDOWS_CERTIFICATE_PASSWORD` | its password |

The certificate is imported into the runner's user store and passed to Tauri by thumbprint. Without the secrets the installer is unsigned and SmartScreen warns. **This path has not yet run with a real certificate.**

### macOS

| Secret | Value |
|---|---|
| `APPLE_CERTIFICATE` | base64 of the Developer ID Application `.p12` |
| `APPLE_CERTIFICATE_PASSWORD` | its password |
| `APPLE_SIGNING_IDENTITY` | e.g. `Developer ID Application: Name (TEAMID)` |
| `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` | Apple ID, app-specific password and team for notarization |

All six are required; then Tauri signs and notarizes. Without them the app carries an ad-hoc signature only (`signingIdentity: "-"`), and users must allow the first launch manually. **This path has not yet run with real credentials.**

## Local helpers

```sh
node scripts/release.mjs check-version 1.0.0
node scripts/release.mjs verify <folder with the six installers>
node scripts/release.mjs notes <folder> 1.0.0 notes.md
node scripts/readme-assets.mjs      # regenerate README artwork
node scripts/screenshots.mjs        # regenerate screenshots (needs `vite preview` and Playwright)
```
