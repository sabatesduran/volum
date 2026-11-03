import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./visual-tests",
  fullyParallel: true,
  forbidOnly: Boolean(process.env.CI),
  retries: process.env.CI ? 2 : 0,
  reporter: process.env.CI ? "github" : "list",
  snapshotPathTemplate: "{testDir}/__screenshots__/{arg}{ext}",
  expect: {
    toHaveScreenshot: {
      animations: "disabled",
      caret: "hide",
      maxDiffPixelRatio: 0.0001,
    },
  },
  use: {
    ...devices["Desktop Chrome"],
    baseURL: "http://127.0.0.1:1430",
    colorScheme: "light",
    locale: "en-US",
    timezoneId: "UTC",
    reducedMotion: "reduce",
    serviceWorkers: "block",
  },
  projects: [{ name: "mobile-chromium", use: { browserName: "chromium" } }],
  webServer: {
    command: "npm run dev -- --host 127.0.0.1",
    url: "http://127.0.0.1:1430/visual.html",
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
});
