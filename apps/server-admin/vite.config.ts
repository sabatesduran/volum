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
    manifest: true,
    outDir: "dist",
    emptyOutDir: true,
    target: ["es2021", "safari14"],
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (id.includes("/node_modules/react/") || id.includes("/node_modules/react-dom/") || id.includes("/node_modules/scheduler/")) return "react-runtime";
          if (id.includes("/node_modules/lucide-react/")) return "icons";
        },
      },
    },
  },
});
