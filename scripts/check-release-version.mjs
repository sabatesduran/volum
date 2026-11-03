import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { resolve } from "node:path";

const root = fileURLToPath(new URL("../", import.meta.url));

async function jsonVersion(path) {
  return JSON.parse(await readFile(resolve(root, path), "utf8")).version;
}

async function cargoVersion(path) {
  const source = await readFile(resolve(root, path), "utf8");
  const packageSection = source.match(/^\[package\]\s*\n([\s\S]*?)(?=\n\[|$)/)?.[1] ?? "";
  const version = packageSection.match(/^version\s*=\s*"([^"]+)"\s*$/m)?.[1];
  if (!version) throw new Error(`Could not read package version from ${path}`);
  return version;
}

const canonical = await jsonVersion("apps/desktop/src-tauri/tauri.conf.json");
const versions = new Map([
  ["package.json", await jsonVersion("package.json")],
  ["apps/desktop/package.json", await jsonVersion("apps/desktop/package.json")],
  ["apps/desktop/src-tauri/Cargo.toml", await cargoVersion("apps/desktop/src-tauri/Cargo.toml")],
  ["apps/mobile/package.json", await jsonVersion("apps/mobile/package.json")],
  ["apps/mobile/src-tauri/tauri.conf.json", await jsonVersion("apps/mobile/src-tauri/tauri.conf.json")],
  ["apps/mobile/src-tauri/Cargo.toml", await cargoVersion("apps/mobile/src-tauri/Cargo.toml")],
  ["apps/server-admin/package.json", await jsonVersion("apps/server-admin/package.json")],
  ["apps/website/package.json", await jsonVersion("apps/website/package.json")],
  ["apps/volumd/Cargo.toml", await cargoVersion("apps/volumd/Cargo.toml")],
  ["crates/volum-server/Cargo.toml", await cargoVersion("crates/volum-server/Cargo.toml")],
  ["packages/companion-api/package.json", await jsonVersion("packages/companion-api/package.json")],
  ["packages/design/package.json", await jsonVersion("packages/design/package.json")],
  ["packages/photo-room/package.json", await jsonVersion("packages/photo-room/package.json")],
  ["packages/shared/package.json", await jsonVersion("packages/shared/package.json")],
  ["packages/viewer/package.json", await jsonVersion("packages/viewer/package.json")],
]);

const failures = [...versions]
  .filter(([, version]) => version !== canonical)
  .map(([path, version]) => `${path} is ${version}; expected ${canonical}`);
const changelog = await readFile(resolve(root, "CHANGELOG.md"), "utf8");
if (!changelog.includes(`## [${canonical}]`)) {
  failures.push(`CHANGELOG.md has no release section for ${canonical}`);
}

const tagIndex = process.argv.indexOf("--tag");
if (tagIndex >= 0) {
  const tag = process.argv[tagIndex + 1];
  if (tag !== `v${canonical}`) failures.push(`release tag ${tag || "<missing>"} does not match v${canonical}`);
}

if (failures.length) {
  console.error(failures.join("\n"));
  process.exit(1);
}
console.log(`Release version ${canonical} is consistent across ${versions.size + 1} manifests and the changelog.`);
