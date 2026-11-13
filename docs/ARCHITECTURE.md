# Architecture (iteration 1)

```
React/TS UI  ──invoke(typed)──▶  src-tauri (Rust)  ──▶  easytizen-core  ──argv──▶  bundled node
 (no process,                     validates args,        pure logic, tested        └▶ vendored Samsung
  no fs access)                   settings, state                                      tizen-sdk-skills CLI ──▶ sdb ──▶ TV
```

* `crates/easytizen-core` — validation, safe WGT analysis, typed Samsung CLI invocations, envelope parsing,
  diagnostics, TCP pre-checks. No Tauri dependency; fully unit-tested.
* `src-tauri` — thin command layer (`api.rs`), non-secret settings, toolchain location.
* `resources/samsung-cli` — Samsung `tizen-sdk-skills` `common/lib` + TV CA certs, pinned (see `PINNED.json`),
  Apache-2.0 (LICENSE + NOTICE shipped).
* `resources/node` — filled by `scripts/fetch-node.mjs` (official Node 22, SHA-256 verified) at build time.

## Facts about the Samsung CLI that shaped the design (verified from source / by running it)

1. It is a set of Node.js scripts, **Node ≥ 22** (18–21 tolerated with a warning).
2. **`TIZEN_SDK_PATH` is ignored.** The SDK path is read from `<HOME>/.tizen.sdk.path.config`, written by
   `sdk-init-cli.js <path>`. TV Deploy therefore runs every script with a **private HOME/USERPROFILE**
   (`<app-data>/samsung-home`), so a user's own Tizen Studio config is never touched. Consequence: certificates and
   profiles created by TV Deploy live in that private home (import covers existing ones — iteration 2).
3. Output is a JSON "envelope" on stdout (`success|failure|needs_action`, plus a legacy `error` shape from
   `cert-manager-cli.js`); progress lines go to stderr. The parser tolerates noise.
4. LAN scan sweeps a /24 for an **open SDB port (26101)** — a TV is only found when Developer Mode is on.
5. Passwords are never accepted safely as arguments by our design; Samsung offers hidden prompt / password-file
   mechanisms (`hidden-password.js`, `password-file.js`) — to be used in iteration 2.
