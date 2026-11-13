# Test report — Public Preview 0.1.0

## Automated tests present in the repository

The project includes Rust unit/integration tests for input validation, WGT parsing and archive safety, Samsung CLI envelope parsing, diagnostics, redaction and process execution. It also includes a scripted end-to-end check which runs the vendored Samsung CLI scripts against a mock SDB executable, plus Vitest checks for localization and frontend utilities.

The earlier development report recorded:
- Rust workspace: 38 tests passed in the authoring Linux environment.
- Frontend Vitest: 6 tests passed.
- The mock-SDB test exercised the real vendored Samsung scripts, but **did not exercise a physical television**.

## CI failures found during integration

The latest development commit before this PR (`a492e58`) had four CI issues, now corrected in this PR:
- Rust formatting differences in `tvdeploy-core/src/samsung/commands.rs`.
- An unused React icon import, rejected by TypeScript's no-unused-locals check.
- Missing `tempfile` dev-dependency for Tauri crate tests.
- A moved-value error in the toolchain status response.

The PR's live GitHub Actions checks are the authoritative validation for this patched tree. Do not treat the report as a pass until those checks complete successfully.

## Not tested yet

- Physical Samsung TV connection, DUID, Samsung Account OAuth, online certificate issuance, signing and installation.
- Visual interaction in a packaged desktop window.
- Windows/macOS/Linux installers on real operating systems.
- macOS Intel runner packaging on the replacement GitHub Actions image.

See [ACCEPTANCE-CHECKLIST.md](ACCEPTANCE-CHECKLIST.md) for the required manual verification.
