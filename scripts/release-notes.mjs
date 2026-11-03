import { readFile } from "node:fs/promises";

const version = process.argv[2];
if (!version) throw new Error("Usage: node scripts/release-notes.mjs <version>");

const changelog = await readFile(new URL("../CHANGELOG.md", import.meta.url), "utf8");
const escaped = version.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
const heading = new RegExp(`^## \\[${escaped}\\][^\\n]*\\n`, "m").exec(changelog);
const remainder = heading ? changelog.slice(heading.index + heading[0].length) : "";
const boundaries = [remainder.search(/^## \[/m), remainder.search(/^\[[^\n]+\]:/m)]
  .filter((index) => index >= 0);
const section = remainder.slice(0, boundaries.length ? Math.min(...boundaries) : undefined).trim();
if (!section) throw new Error(`CHANGELOG.md has no release notes for ${version}`);

console.log(`Volum for macOS, Linux, and Windows, with a visual, local-first home for your 3D-printing projects.

## What's new in ${version}

${section}

Windows installers are not yet Authenticode-signed, so Microsoft Defender SmartScreen may show a warning on first install. In-app update packages are signed and verified by Volum.`);
