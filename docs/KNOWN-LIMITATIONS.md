# Known limitations — EasyTizen 0.9

This page separates what is implemented from what has been confirmed, and lists real limits.

## Not yet confirmed on hardware

1. **No physical Samsung TV was available.** Connection, DUID retrieval, installation and launch were verified against a protocol-level TV emulator (`crates/easytizen-core/src/sdb/mock.rs`) written from the SDB protocol and Samsung's tools — not against a television. The exact device commands (`0 getduid`, `0 vd_appinstall`, `0 was_execute`) are the ones Samsung's tooling uses, but their behaviour on specific models and firmware years is unverified.
2. **No real Android device.** The APK builds (arm64 verified locally; armv7 in CI) and its manifest was inspected, but the app has not been run on a phone. In particular: the document picker → backend hand-off, the system browser round trip for Samsung sign-in, and LAN access on Android 14–16.
3. **Samsung's services were not contacted.** OAuth and certificate issuance were tested against a local stand-in whose CA role is played by the OpenSSL CLI. The request shapes follow Samsung's open-source CLI; a real Samsung Account sign-in and a real certificate were not exercised.
4. **Windows and macOS installers** are built only in CI; they have not been installed and run by hand. The Linux `.deb`/AppImage were built locally and the binary was started under Xvfb.
5. **The SDK engine's end-to-end path** (official `tz pack` signing and Samsung CLI install) still relies on the mock-`sdb` test inherited from 0.1 — no real SDK installation (15 GB) was performed in the authoring environment.

## Real limits of this version

* **Built-in engine is experimental.** Its signatures are verified by `xmlsec1`, but a TV is the final judge. Desktop defaults to the official Samsung tools for that reason.
* **Android 17 (API 37)** makes local-network access a runtime permission for apps that target it. EasyTizen targets API 36, where this does not apply. The permission flow must be added before raising the target.
* **Samsung sign-in on Android** returns to `http://localhost:<port>`. If the phone freezes background apps aggressively, the browser page keeps loading until you switch back to EasyTizen.
* **TV discovery** scans one /24 subnet of the primary interface. Larger or segmented networks need the TV's address typed in.
* **One connection at a time** in the built-in SDB client; no persistent connection is kept between operations.
* **Certificate password** (built-in engine) is asked once per app session and never stored. A forgotten password cannot be recovered — create a new certificate.
* **Certificates are per engine.** Moving between the SDK keystore and the built-in store means importing the `.p12` files.
* **Uninstalling an app from the TV** from within EasyTizen is available in the built-in engine only.
* **macOS + official tools** needs Bash 4+ (`brew install bash`); EasyTizen reports this but does not install it.
* **Signature check before installation is structural** (both signature documents present and well-formed), not a trust-chain validation.
* **Update check only.** EasyTizen links to the new installer; it does not download or install updates by itself.
* **Code signing** of Windows/macOS builds and a stable Android key depend on repository secrets (see RELEASING.md). Without them: unsigned Windows installer, ad-hoc signed macOS app, Android APK signed with a per-build throw-away key.
* **Node.js runtime integrity** (desktop) is checked against the SHA-256 published over HTTPS by nodejs.org; the GPG signature of that list is not verified.
* **Only `.wgt`** packages are supported.

## Release gate

`1.0.0` must not be published until the hands-on [acceptance checklist](ACCEPTANCE-CHECKLIST.md) is completed on at least one physical Samsung TV with both the desktop and the Android app.
