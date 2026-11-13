# Known limitations and open problems (end of iteration 1)

## Critical (must be resolved before the next iteration builds on them)
1. **No automatic Tizen SDK install.** Connecting needs `sdb`, which comes from the SDK. Today the user must point to an
   existing SDK. The Samsung `sdk-install-cli.js` / `tv-sdk-install-cli.js` only *pre-check and return an installer
   command* (two-phase design for agents); wrapping that safely (progress, cancel, resume, integrity) is open work and
   the first item of iteration 2 — certificates cannot work without the SDK's certificate tooling anyway.
2. **Nothing was tested against a real TV.** Every connect/DUID path was verified only against a *mock `sdb`* driven
   through the real Samsung scripts. DUID extraction, Developer-Mode diagnosis and Host-PC-IP diagnosis are unverified on hardware.
3. **UI never rendered in a real window here.** Rust and TypeScript compile and unit tests pass, but the sandbox had
   no display; layout, dark theme and drag-and-drop need a visual pass on all three OSes.
4. **Windows/macOS untested.** Only Linux compiled here. CI matrix exists but has not run.
5. **Bundled Node.js** is fetched by `scripts/fetch-node.mjs` in CI (SHA-256 against nodejs.org). That script could not be
   run in the sandbox. GPG verification of `SHASUMS256.txt` is TODO.

## Other
* Install button is intentionally disabled (signing/installation = iteration 3). Not faked.
* Signature check is *structural* only (presence + well-formed XML). It does not verify cryptography or DUID binding.
* LAN scan finds only TVs with the SDB port open (Developer Mode on). TVs with it off will not appear — by design of the Samsung scanner.
* Private HOME for Samsung scripts (see ARCHITECTURE) means existing Tizen Studio certificates are not visible until the import feature (iteration 2).
* Cancelling kills the Node process; a child `sdb` it spawned may linger briefly (no process-group kill yet).
* Frontend supports `.wgt` only; `.tpk/.rpk` exist in the Samsung CLI but are not claimed as supported.
* No code signing / notarization yet; no release workflow yet (iteration 4). No auto-update check yet.
* Rename/remove of saved TVs lives in Settings; the full device manager is iteration 3.
