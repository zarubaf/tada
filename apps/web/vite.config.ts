import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

// The API runs on `tada serve` (TADA_PORT 8080). In production, serve delivers the built files itself (ADR 0005).
export default defineConfig({
  plugins: [react()],
  server: {
    proxy: { "/api": "http://127.0.0.1:8080" },
    // The Fluent files live in the shared `locales/` folder of the repository (ADR 0005).
    fs: { allow: ["../.."] },
  },
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.{ts,tsx}"],
    setupFiles: ["src/test/setup.ts"],
    // The first test of a file is slow while the machine runs the other checks.
    testTimeout: 15_000,
  },
});
