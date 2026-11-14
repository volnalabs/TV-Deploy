#!/usr/bin/env node
// Downloads the pinned official Node.js runtime for a target platform into resources/node/
// and verifies its SHA-256 against nodejs.org's SHASUMS256.txt. Run in CI before `tauri build`.
// Usage: node scripts/fetch-node.mjs <win-x64|darwin-arm64|darwin-x64|linux-x64>
// The GPG signature of SHASUMS256.txt is not verified (see docs/KNOWN-LIMITATIONS.md).
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync, rmSync, copyFileSync, chmodSync, readdirSync } from "node:fs";
import { join } from "node:path";

const NODE_VERSION = "22.23.4"; // Node 22 LTS line; bump deliberately
const target = process.argv[2];
const map = {
  "win-x64": { file: `node-v${NODE_VERSION}-win-x64.zip`, bin: "node.exe" },
  "darwin-arm64": { file: `node-v${NODE_VERSION}-darwin-arm64.tar.gz`, bin: "bin/node" },
  "darwin-x64": { file: `node-v${NODE_VERSION}-darwin-x64.tar.gz`, bin: "bin/node" },
  "linux-x64": { file: `node-v${NODE_VERSION}-linux-x64.tar.xz`, bin: "bin/node" },
};
const spec = map[target];
if (!spec) { console.error("unknown target", target); process.exit(1); }

const base = `https://nodejs.org/dist/v${NODE_VERSION}`;
const get = async (u) => { const r = await fetch(u); if (!r.ok) throw new Error(`${u}: ${r.status}`); return Buffer.from(await r.arrayBuffer()); };

const sums = (await get(`${base}/SHASUMS256.txt`)).toString();
const expected = sums.split("\n").map((l) => l.trim().split(/\s+/)).find(([, f]) => f === spec.file)?.[0];
if (!expected) throw new Error("no checksum published for " + spec.file);
const data = await get(`${base}/${spec.file}`);
const actual = createHash("sha256").update(data).digest("hex");
if (actual !== expected) throw new Error(`SHA-256 mismatch for ${spec.file}`);

const tmp = join("resources", ".node-tmp");
rmSync(tmp, { recursive: true, force: true }); mkdirSync(tmp, { recursive: true });
const archive = join(tmp, spec.file); writeFileSync(archive, data);
// Windows: the .zip needs bsdtar. Call the system one explicitly — in Git Bash the first
// `tar` on PATH is GNU tar, which cannot read zip archives.
const tar = process.platform === "win32" ? join(process.env.SystemRoot ?? "C:\\Windows", "System32", "tar.exe") : "tar";
execFileSync(tar, ["-xf", archive, "-C", tmp], { stdio: ["ignore", "inherit", "inherit"] });
const dir = join(tmp, readdirSync(tmp).find((d) => d.startsWith("node-v") && !d.endsWith(spec.file.split(".").pop())) ?? "");
const out = join("resources", "node"); mkdirSync(out, { recursive: true });
const dest = join(out, target === "win-x64" ? "node.exe" : "node");
copyFileSync(join(dir, spec.bin), dest); if (target !== "win-x64") chmodSync(dest, 0o755);
copyFileSync(join(dir, "LICENSE"), join(out, "NODE-LICENSE"));
rmSync(tmp, { recursive: true, force: true });
console.log(`Node ${NODE_VERSION} (${target}) verified and installed to ${dest}`);
