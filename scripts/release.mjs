#!/usr/bin/env node
// Release helpers used by .github/workflows/release.yml (and testable locally):
//
//   node scripts/release.mjs check-version <version>      versions in the repo match <version>
//   node scripts/release.mjs verify <dir>                 every expected installer is present
//   node scripts/release.mjs notes <dir> <version> <out>  SHA256SUMS.txt + release notes
//
// The file names below are the single source of truth for README links, the update check
// (crates/easytizen-core/src/update.rs) and the release page.
import { createHash } from "node:crypto";
import { existsSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";

export const ASSETS = [
  { file: "EasyTizen-Android.apk", minBytes: 3_000_000 },
  { file: "EasyTizen-Windows-x64-Setup.exe", minBytes: 5_000_000 },
  { file: "EasyTizen-macOS-ARM64.dmg", minBytes: 5_000_000 },
  { file: "EasyTizen-macOS-Intel.dmg", minBytes: 5_000_000 },
  { file: "EasyTizen-Linux-x64.AppImage", minBytes: 5_000_000 },
  { file: "EasyTizen-Linux-x64.deb", minBytes: 5_000_000 },
];

export function parseVersion(text) {
  const m = /^v?(\d+)\.(\d+)\.(\d+)$/.exec(String(text).trim());
  return m ? `${m[1]}.${m[2]}.${m[3]}` : null;
}

/** Versions declared in the repository; all must agree before a release is cut. */
export function repoVersions(root = ".") {
  const pkg = JSON.parse(readFileSync(join(root, "package.json"), "utf8")).version;
  const tauri = JSON.parse(readFileSync(join(root, "src-tauri/tauri.conf.json"), "utf8")).version;
  const cargo = /\[workspace\.package\][^[]*?version\s*=\s*"([^"]+)"/s.exec(readFileSync(join(root, "Cargo.toml"), "utf8"))?.[1];
  return { "package.json": pkg, "src-tauri/tauri.conf.json": tauri, "Cargo.toml": cargo };
}

export function checkVersion(version, root = ".") {
  const wanted = parseVersion(version);
  if (!wanted) return [`"${version}" is not a version like 1.2.3`];
  return Object.entries(repoVersions(root))
    .filter(([, v]) => v !== wanted)
    .map(([file, v]) => `${file} declares ${v}, expected ${wanted}`);
}

/** Problems with the collected installers: missing, suspiciously small or unexpected files. */
export function verifyAssets(dir) {
  const problems = [];
  const present = existsSync(dir) ? readdirSync(dir) : [];
  for (const { file, minBytes } of ASSETS) {
    if (!present.includes(file)) problems.push(`missing: ${file}`);
    else if (statSync(join(dir, file)).size < minBytes) problems.push(`too small to be a real installer: ${file}`);
  }
  for (const file of present) {
    if (!ASSETS.some((a) => a.file === file) && file !== "SHA256SUMS.txt") problems.push(`unexpected file: ${file}`);
  }
  return problems;
}

export function sha256Of(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

/** The section of CHANGELOG.md for `version` (without its heading), or null. */
export function changelogSection(changelog, version) {
  const lines = changelog.split(/\r?\n/);
  const start = lines.findIndex((l) => new RegExp(`^## \\[?${version.replaceAll(".", "\\.")}\\]?(\\s|$)`).test(l));
  if (start < 0) return null;
  const rest = lines.slice(start + 1);
  const end = rest.findIndex((l) => /^## /.test(l));
  return (end < 0 ? rest : rest.slice(0, end)).join("\n").trim();
}

export function renderNotes({ template, version, changes, sums, signing }) {
  const table = ["| Файл | SHA-256 |", "|---|---|", ...sums.map(({ file, sha }) => `| \`${file}\` | \`${sha}\` |`)].join("\n");
  const signingNotes = [
    signing.android === "release"
      ? "- **Android:** APK подписан постоянным ключом проекта — обновление ставится поверх предыдущей версии."
      : "- **Android:** APK подписан временным ключом этой сборки. Чтобы установить его поверх другой версии EasyTizen, сначала удалите старую (сертификаты при этом удаляются — экспортируйте или создайте их заново).",
    signing.windows === "signed"
      ? "- **Windows:** установщик подписан сертификатом издателя."
      : "- **Windows:** установщик не подписан сертификатом издателя — SmartScreen покажет предупреждение («Подробнее → Выполнить в любом случае»).",
    signing.macos === "notarized"
      ? "- **macOS:** приложение подписано Developer ID и нотаризовано Apple."
      : "- **macOS:** приложение не нотаризовано Apple (подпись ad-hoc) — первый запуск нужно разрешить вручную, см. инструкцию.",
  ].join("\n");
  return template
    .replaceAll("{{VERSION}}", version)
    .replaceAll("{{CHANGES}}", changes)
    .replaceAll("{{SHA256_TABLE}}", table)
    .replaceAll("{{SIGNING}}", signingNotes);
}

function main(argv) {
  const [command, ...args] = argv;
  if (command === "check-version") {
    const problems = checkVersion(args[0]);
    if (problems.length) { console.error(problems.join("\n")); process.exit(1); }
    console.log(`version ${parseVersion(args[0])} is consistent`);
  } else if (command === "verify") {
    const problems = verifyAssets(args[0]);
    if (problems.length) { console.error(`Release blocked:\n${problems.join("\n")}`); process.exit(1); }
    console.log(`all ${ASSETS.length} installers are present`);
  } else if (command === "notes") {
    const [dir, rawVersion, out] = args;
    const version = parseVersion(rawVersion);
    const problems = verifyAssets(dir);
    if (!version || problems.length) { console.error(["cannot write notes", ...problems].join("\n")); process.exit(1); }
    const sums = ASSETS.map(({ file }) => ({ file, sha: sha256Of(join(dir, file)) }));
    writeFileSync(join(dir, "SHA256SUMS.txt"), sums.map(({ file, sha }) => `${sha}  ${file}`).join("\n") + "\n");
    const changes = changelogSection(readFileSync("CHANGELOG.md", "utf8"), version);
    if (!changes) { console.error(`CHANGELOG.md has no section for ${version}`); process.exit(1); }
    const signing = { android: process.env.SIGNING_ANDROID || "temporary", windows: process.env.SIGNING_WINDOWS || "unsigned", macos: process.env.SIGNING_MACOS || "adhoc" };
    writeFileSync(out, renderNotes({ template: readFileSync(".github/release-notes.md", "utf8"), version, changes, sums, signing }));
    console.log(`wrote ${out} and SHA256SUMS.txt`);
  } else {
    console.error("usage: release.mjs check-version <version> | verify <dir> | notes <dir> <version> <out>");
    process.exit(2);
  }
}

if (process.argv[1] && import.meta.url.endsWith(process.argv[1].replaceAll("\\", "/").split("/").pop())) main(process.argv.slice(2));
