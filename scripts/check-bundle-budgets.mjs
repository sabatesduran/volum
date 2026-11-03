import { readFile, readdir, stat } from "node:fs/promises";
import { resolve } from "node:path";
import { brotliCompressSync, constants, gzipSync } from "node:zlib";
import { fileURLToPath } from "node:url";

const application = process.argv[2];
const budgets = {
  desktop: {
    total: 2_900_000,
    chunk: 720_000,
    initial: 700_000,
    initialGzip: 220_000,
    named: { "three-runtime": 720_000, "viewer-runtime": 420_000, "media-runtime": 420_000, "photo-room": 180_000 },
  },
  mobile: {
    total: 2_600_000,
    chunk: 720_000,
    initial: 650_000,
    initialGzip: 210_000,
    named: { "three-runtime": 720_000, "viewer-runtime": 420_000, "media-runtime": 420_000, "photo-room": 180_000 },
  },
  "server-admin": { total: 420_000, chunk: 280_000, initial: 420_000, initialGzip: 140_000, named: {} },
};

if (!budgets[application]) {
  console.error(`Unknown bundle budget '${application}'.`);
  process.exit(2);
}

const repositoryRoot = fileURLToPath(new URL("../", import.meta.url));
const distributionDirectory = resolve(repositoryRoot, `apps/${application}/dist`);
const assetDirectory = resolve(distributionDirectory, "assets");
const files = (await readdir(assetDirectory)).filter((file) => file.endsWith(".js"));
const sizes = await Promise.all(files.map(async (file) => {
  const path = resolve(assetDirectory, file);
  const contents = await readFile(path);
  return {
    file,
    outputPath: `assets/${file}`,
    bytes: (await stat(path)).size,
    gzip: gzipSync(contents, { level: 9 }).byteLength,
    brotli: brotliCompressSync(contents, {
      params: { [constants.BROTLI_PARAM_QUALITY]: 11 },
    }).byteLength,
  };
}));

const manifest = JSON.parse(await readFile(resolve(distributionDirectory, ".vite/manifest.json"), "utf8"));
const entryPair = Object.entries(manifest).find(([, record]) => record.isEntry);
if (!entryPair) throw new Error(`${application}: Vite manifest has no entry chunk`);
const initialFiles = new Set();
const visited = new Set();
const visitInitial = (key, record) => {
  if (visited.has(key)) return;
  visited.add(key);
  if (record.file?.endsWith(".js")) initialFiles.add(record.file);
  for (const imported of record.imports ?? []) {
    const dependency = manifest[imported];
    if (dependency) visitInitial(imported, dependency);
  }
};
visitInitial(...entryPair);

const budget = budgets[application];
const failures = [];
const total = sizes.reduce((sum, asset) => sum + asset.bytes, 0);
const initialAssets = sizes.filter((asset) => initialFiles.has(asset.outputPath));
const initial = initialAssets.reduce((sum, asset) => sum + asset.bytes, 0);
const initialGzip = initialAssets.reduce((sum, asset) => sum + asset.gzip, 0);
const initialBrotli = initialAssets.reduce((sum, asset) => sum + asset.brotli, 0);

if (total > budget.total) failures.push(`total JavaScript ${total} B exceeds ${budget.total} B`);
if (initial > budget.initial) failures.push(`initial JavaScript ${initial} B exceeds ${budget.initial} B`);
if (initialGzip > budget.initialGzip) failures.push(`initial gzip JavaScript ${initialGzip} B exceeds ${budget.initialGzip} B`);
for (const asset of sizes) {
  if (asset.bytes > budget.chunk) failures.push(`${asset.file} is ${asset.bytes} B; per-chunk limit is ${budget.chunk} B`);
  for (const [prefix, maximum] of Object.entries(budget.named)) {
    if (asset.file.startsWith(`${prefix}-`) && asset.bytes > maximum) {
      failures.push(`${asset.file} is ${asset.bytes} B; ${prefix} limit is ${maximum} B`);
    }
  }
}

console.log(`${application}: ${(total / 1024).toFixed(1)} KiB total; ${(initial / 1024).toFixed(1)} KiB initial (${(initialGzip / 1024).toFixed(1)} KiB gzip, ${(initialBrotli / 1024).toFixed(1)} KiB brotli) across ${sizes.length} chunks`);
if (failures.length) {
  failures.forEach((failure) => console.error(`bundle budget: ${failure}`));
  process.exitCode = 1;
}
