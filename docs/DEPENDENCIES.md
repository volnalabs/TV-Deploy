# Direct dependencies (iteration 1)

## Rust (workspace; full tree: `cargo tree`, lockfile: Cargo.lock)
### tvdeploy-core
- roxmltree ^0.20
- serde ^1
- serde_json ^1
- thiserror ^2
- zip ^2
- tempfile ^3 (dev)
### tv-deploy
- serde ^1
- serde_json ^1
- tauri ^2
- tauri-plugin-dialog ^2
- tvdeploy-core *
- tauri-build ^2 (build)

## npm (lockfile: package-lock.json)
- @tauri-apps/api ^2 (runtime)
- @tauri-apps/plugin-dialog ^2 (runtime)
- lucide-react ^0.468.0 (runtime)
- react ^18.3.1 (runtime)
- react-dom ^18.3.1 (runtime)
- @tailwindcss/vite ^4 (dev)
- @tauri-apps/cli ^2 (dev)
- @types/react ^18.3.12 (dev)
- @types/react-dom ^18.3.1 (dev)
- @vitejs/plugin-react ^4.3.4 (dev)
- tailwindcss ^4 (dev)
- typescript ^5.6.3 (dev)
- vite ^6 (dev)
- vitest ^2 (dev)

## Vendored
- Samsung tizen-sdk-skills common/lib @ 1509f7b9c5f659dda2222ce4264a3a3a35cbbe69 (Apache-2.0) — see resources/samsung-cli/PINNED.json
- Node.js 22.23.4 (fetched at build time, MIT) — scripts/fetch-node.mjs
