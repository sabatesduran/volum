import { configDefaults, defineConfig } from "vitest/config";

export default defineConfig({
  resolve: { dedupe: ["react", "react-dom", "three", "three-mesh-bvh"] },
  test: {
    exclude: [
      ...configDefaults.exclude,
      "**/visual-tests/**",
      "**/playwright/**"
    ]
  }
});
