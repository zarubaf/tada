import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

// The API and the MCP server run on `tada serve` (TADA_PORT 8080). In production, serve delivers the built files itself (ADR 0005).
// An MCP client in development uses `http://localhost:5173/mcp`, the URL that the token page shows (ADR 0040).
export default defineConfig({
  plugins: [react()],
  server: {
    proxy: { "/api": "http://127.0.0.1:8080", "/mcp": "http://127.0.0.1:8080" },
    // The Fluent files live in the shared `locales/` folder of the repository (ADR 0005).
    fs: { allow: ["../.."] },
  },
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.{ts,tsx}"],
    setupFiles: ["src/test/setup.ts"],
    // The timeout guards against a hang, not against slow code: parallel checks load the machine.
    testTimeout: 15_000,
  },
});
