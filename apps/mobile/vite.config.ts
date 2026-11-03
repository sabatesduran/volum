import react from "@vitejs/plugin-react";
import { configDefaults, defineConfig } from "vitest/config";

const host = process.env.TAURI_DEV_HOST;

function manualChunks(id: string) {
  if (id.includes("/node_modules/three/examples/")) return "three-addons";
  if (id.includes("/node_modules/three/")) return "three-runtime";
  if (id.includes("/node_modules/@react-three/") || id.includes("/node_modules/three-mesh-bvh/") || id.includes("/packages/viewer/")) return "viewer-runtime";
  if (id.includes("/node_modules/mediabunny/")) return "media-runtime";
  if (id.includes("/packages/photo-room/")) return "photo-room";
  if (id.includes("/node_modules/@tanstack/")) return "query-runtime";
  if (id.includes("/node_modules/react/") || id.includes("/node_modules/react-dom/") || id.includes("/node_modules/scheduler/")) return "react-runtime";
  if (id.includes("/node_modules/lucide-react/")) return "icons";
}

export default defineConfig({
  plugins: [react()],
  resolve: { dedupe: ["react", "react-dom", "three", "three-mesh-bvh"], preserveSymlinks: true },
  clearScreen: false,
  test: {
    exclude: [...configDefaults.exclude, "visual-tests/**"],
  },
  server: {
    port: 1430,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1431 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] }
  },
  build: {
    manifest: true,
    target: ["es2021", "safari14"],
    minify: process.env.TAURI_ENV_DEBUG ? false : "esbuild",
    sourcemap: Boolean(process.env.TAURI_ENV_DEBUG),
    chunkSizeWarningLimit: 720,
    rollupOptions: { output: { manualChunks } }
  }
});
