import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import react from "@vitejs/plugin-react";
import { defineConfig, type Plugin } from "vitest/config";

// The policy of the built client. `tada serve` sends the same file with the web files (ADR 0058).
const policy = readFileSync(
  new URL("./content-security-policy.txt", import.meta.url),
  "utf8",
).trim();

// The development server needs two more rights: React Refresh starts with an inline script, and
// hot reload talks over a WebSocket.
function widen(text: string, directive: string, extra: string): string {
  if (!text.includes(directive)) {
    throw new Error(`content-security-policy.txt has no "${directive}" to widen for development`);
  }
  return text.replace(directive, `${directive} ${extra}`);
}

const developmentPolicy = widen(
  widen(policy, "script-src 'self'", "'unsafe-inline'"),
  "connect-src 'self'",
  "ws:",
);

// The build joins the CSS of all chunks, shared chunks first, so a layer block of a component
// would come before the layer order and fix a wrong rank. The statement starts the built CSS.
const layerOrder = readFileSync(new URL("./src/styles/layers.css", import.meta.url), "utf8")
  .replace(/\/\*[\s\S]*?\*\//g, "")
  .trim();

function layerOrderFirst(): Plugin {
  return {
    name: "tada-layer-order-first",
    apply: "build",
    enforce: "post",
    generateBundle(_options, bundle) {
      for (const file of Object.values(bundle)) {
        if (file.type === "asset" && file.fileName.endsWith(".css")) {
          // Vite gives CSS as a string; a byte array would turn into numbers with `toString`.
          const css =
            typeof file.source === "string" ? file.source : new TextDecoder().decode(file.source);
          file.source = `${layerOrder}${css}`;
        }
      }
    },
  };
}

// The build identity (ADR 0075). The commit has seven characters, as the one of the binary.
function buildCommit(): string {
  const given = process.env.TADA_BUILD_COMMIT?.trim();
  if (given) return given.slice(0, 7);
  try {
    return execFileSync("git", ["rev-parse", "--short=7", "HEAD"], { encoding: "utf8" }).trim();
  } catch {
    return "unknown";
  }
}

// The build time as `YYYY-MM-DDTHH:MM:SSZ`; SOURCE_DATE_EPOCH fixes it, as it does for the binary.
function builtAt(): string {
  const epoch = Number(process.env.SOURCE_DATE_EPOCH);
  const date =
    Number.isFinite(epoch) && process.env.SOURCE_DATE_EPOCH ? new Date(epoch * 1000) : new Date();
  return date.toISOString().replace(/\.\d{3}Z$/, "Z");
}

const { version } = JSON.parse(
  readFileSync(new URL("./package.json", import.meta.url), "utf8"),
) as {
  version: string;
};

// The build identity in the client. The browser checks build in the mode `e2e`; their screenshots
// show the footer, so that build has a fixed identity that does not change with each commit.
function defines(mode: string) {
  const identity =
    mode === "e2e"
      ? { version: "0.0.0", commit: "1a2b3c4", builtAt: "2030-01-01T00:00:00Z" }
      : { version, commit: buildCommit(), builtAt: builtAt() };
  return {
    __TADA_VERSION__: JSON.stringify(identity.version),
    __TADA_COMMIT__: JSON.stringify(identity.commit),
    __TADA_BUILT_AT__: JSON.stringify(identity.builtAt),
  };
}

// The API and the MCP server run on `tada serve` (TADA_PORT 8080). In production, serve delivers the built files itself (ADR 0005).
// An MCP client in development uses `http://localhost:5173/mcp`, the URL that the token page shows (ADR 0040).
export default defineConfig(({ mode }) => ({
  plugins: [react(), layerOrderFirst()],
  define: defines(mode),
  // One CSS file for all chunks: the settings pages load on demand, and the order of the CSS layers
  // must stay the same as without the split.
  build: { cssCodeSplit: false },
  server: {
    proxy: { "/api": "http://127.0.0.1:8080", "/mcp": "http://127.0.0.1:8080" },
    headers: { "Content-Security-Policy": developmentPolicy },
    // The Fluent files live in the shared `locales/` folder of the repository (ADR 0005).
    fs: { allow: ["../.."] },
  },
  // The browser checks run the built client here, so they test the real policy.
  preview: { headers: { "Content-Security-Policy": policy } },
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.{ts,tsx}"],
    setupFiles: ["src/test/setup.ts"],
    // The timeout guards against a hang, not against slow code: parallel checks load the machine.
    testTimeout: 15_000,
  },
}));
