import { defineConfig } from "vite";
import { readFileSync } from "node:fs";
import { fileURLToPath, URL } from "node:url";

const packageMetadata = JSON.parse(readFileSync(fileURLToPath(new URL("../package.json", import.meta.url)), "utf8"));

export default defineConfig({
  root: fileURLToPath(new URL(".", import.meta.url)),
  base: "./",
  plugins: [{
    name: "volum-version",
    transformIndexHtml: {
      order: "pre",
      handler: (html) => html.replaceAll("__VOLUM_VERSION__", packageMetadata.version)
    }
  }],
  build: {
    assetsInlineLimit: 0,
    outDir: fileURLToPath(new URL("./dist", import.meta.url)),
    emptyOutDir: true
  }
});
