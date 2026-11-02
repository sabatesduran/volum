import react from "@vitejs/plugin-react";
import { fileURLToPath, URL } from "node:url";
import { defineConfig } from "vite";

export default defineConfig({
  root: fileURLToPath(new URL(".", import.meta.url)),
  plugins: [react()],
  server: {
    port: 1432,
    strictPort: true,
    proxy: {
      "/api": "http://127.0.0.1:43821",
      "/identity": "http://127.0.0.1:43821",
      "/pair": "http://127.0.0.1:43821",
    },
  },
  build: {
    outDir: "../server-web-dist",
    emptyOutDir: true,
    target: ["es2021", "safari14"],
  },
});
