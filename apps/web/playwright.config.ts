// Browser checks of ADR 0024. They run in the pinned Playwright container (`mise run check:browser`),
// so that the screenshots do not depend on the fonts of a laptop.
import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "e2e",
  snapshotPathTemplate: "e2e/screenshots/{testFilePath}/{arg}{ext}",
  fullyParallel: true,
  forbidOnly: true,
  reporter: [["list"]],
  expect: { toHaveScreenshot: { maxDiffPixelRatio: 0.001 } },
  use: { baseURL: "http://127.0.0.1:4173", ...devices["Desktop Chrome"] },
  webServer: {
    // The e2e build has the gallery and the pseudo-locale; the production build has neither.
    command:
      "node_modules/.bin/vite build --mode e2e --outDir dist-e2e && node_modules/.bin/vite preview --mode e2e --outDir dist-e2e --host 127.0.0.1 --port 4173 --strictPort",
    url: "http://127.0.0.1:4173",
    reuseExistingServer: false,
  },
});
