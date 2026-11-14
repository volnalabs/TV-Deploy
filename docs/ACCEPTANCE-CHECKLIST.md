# Hands-on acceptance checklist

**Status: NOT YET EXECUTED.** Record the TV model and year, Tizen version, host device and OS, EasyTizen build and engine for every run. Do not tick a box on the strength of automated tests.

## A. Android app (built-in engine) — no computer involved

- [ ] APK installs on Android 7–16; app starts; welcome guide shows the phone's Wi-Fi address.
- [ ] "Find TVs" discovers a TV with Developer Mode on; progress bar moves; Cancel works.
- [ ] Connect by IP works; the TV's DUID and model are shown.
- [ ] Wrong IP / Developer Mode off / wrong Host PC IP each produce the matching explanation.
- [ ] Picking a `.wgt` through the system picker shows its details; a non-WGT file is rejected with a clear message.
- [ ] "Create certificate" opens Samsung sign-in in the browser; after signing in and returning to the app the certificate appears. Note whether the app had to be brought to the foreground manually.
- [ ] Cancelling during sign-in returns to the form without leftovers; retrying works.
- [ ] Install: stages advance, upload percentage moves, the app appears on the TV and launches.
- [ ] After restarting the app, installing asks for the certificate password once; a wrong password is reported as such.
- [ ] A package signed for TV A is refused by TV B with the "signature rejected" message.
- [ ] Installing over an app signed by another author offers "Remove the old copy…", which works.
- [ ] Importing `author.p12` + `distributor.p12` exported from Tizen Studio works.
- [ ] Rotation, dark/light theme, Russian/English, back gesture, small screens.

## B. Desktop — official Samsung tools

- [ ] Installer works on Windows 10/11, macOS (Apple silicon and Intel), Ubuntu/Debian (`.deb`) and one more distribution (AppImage).
- [ ] "Install automatically": folder and free space shown; progress by package; Cancel stops all downloads; "Resume" finishes the installation.
- [ ] Too little disk space and a path with spaces are refused before anything is downloaded.
- [ ] "Use an existing SDK" accepts a Tizen Studio folder and leaves the user's own Tizen configuration untouched.
- [ ] Scan, connect, DUID, certificate creation (browser sign-in), signing with `tz`, installation and launch all succeed.
- [ ] Certificate import through the Samsung certificate manager works.
- [ ] Upgrade from TV Deploy 0.1: saved TVs, SDK path and profiles are present after first start.

## C. Desktop — built-in engine

- [ ] Switching the engine in Settings works; certificates of the other engine are shown as unavailable.
- [ ] The same end-to-end flow as in section A succeeds from a computer.

## D. Cross-cutting

- [ ] Original `.wgt` is byte-identical after installation (compare SHA-256).
- [ ] History lists successes and failures with the TV's message; Clear works.
- [ ] Diagnostics report contains no passwords, tokens or full DUIDs; Copy and Save work.
- [ ] Update banner appears when a newer release exists and links to the right installer.
- [ ] Release workflow publishes exactly six installers plus `SHA256SUMS.txt`; every `releases/latest/download/…` link in the README downloads a file.
