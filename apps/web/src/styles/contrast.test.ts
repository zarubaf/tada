// @vitest-environment node
// The checked contrast pairs of doc/design/tokens.md, computed from tokens.css (ADR 0022, ADR 0082).
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./tokens.css", import.meta.url), "utf8");

/** The declarations of the first block that starts with `selector`, as a map of name to value. */
function block(selector: string, from = 0): Map<string, string> {
  const start = css.indexOf(`${selector} {`, from);
  if (start < 0) {
    throw new Error(`tokens.css has no block for ${selector}`);
  }
  const end = css.indexOf("}", start);
  const tokens = new Map<string, string>();
  for (const match of css.slice(start, end).matchAll(/(--[\w-]+):\s*([^;]+);/g)) {
    tokens.set(match[1] as string, (match[2] as string).trim());
  }
  return tokens;
}

const light = block(":root");
const dark = block(':root[data-theme="dark"]');
// The system preference has its own block with the same values as the explicit choice.
const darkBySystem = block(
  ':root:not([data-theme="light"])',
  css.indexOf("prefers-color-scheme: dark"),
);

type Rgb = [number, number, number];

function oklchToLinearRgb(value: string): Rgb {
  const match = /^oklch\(([\d.]+)%\s+([\d.]+)\s+([\d.]+)\)$/.exec(value);
  if (!match) {
    throw new Error(`not an opaque oklch color: ${value}`);
  }
  const l = Number(match[1]) / 100;
  const c = Number(match[2]);
  const h = (Number(match[3]) * Math.PI) / 180;
  const a = c * Math.cos(h);
  const b = c * Math.sin(h);
  const l_ = (l + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m_ = (l - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s_ = (l - 0.0894841775 * a - 1.291485548 * b) ** 3;
  const rgb: Rgb = [
    4.0767416621 * l_ - 3.3077115913 * m_ + 0.2309699292 * s_,
    -1.2684380046 * l_ + 2.6097574011 * m_ - 0.3413193965 * s_,
    -0.0041960863 * l_ - 0.7034186147 * m_ + 1.707614701 * s_,
  ];
  return rgb.map((channel) => Math.min(1, Math.max(0, channel))) as Rgb;
}

function luminance(value: string): number {
  const [r, g, b] = oklchToLinearRgb(value);
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** The WCAG 2.2 contrast ratio of two colors. */
function ratio(foreground: string, background: string): number {
  const [high, low] = [luminance(foreground), luminance(background)].sort((x, y) => y - x);
  return ((high as number) + 0.05) / ((low as number) + 0.05);
}

const TEXT = 4.5;
const AVATARS = [1, 2, 3, 4, 5, 6].map((n) => `avatar-${n}`);
const UI = 3;
const surfaces = ["bg-canvas", "bg-surface", "bg-raised", "bg-sunken", "bg-hover", "accent-subtle"];

/** [foreground, background, minimum]. Names are token names without `--color-`. */
const pairs: [string, string, number][] = [
  ...surfaces.map((bg): [string, string, number] => ["text", bg, TEXT]),
  ...surfaces.map((bg): [string, string, number] => ["text-muted", bg, TEXT]),
  // The neutral fill of a selected option of a segmented switch or a type filter.
  ["text", "bg-active", TEXT],
  ["text-muted", "bg-active", TEXT],
  ["text-on-accent", "accent", TEXT],
  ["text-on-accent", "accent-hover", TEXT],
  ...["bg-canvas", "bg-surface", "bg-raised", "bg-sunken", "accent-subtle"].map(
    (bg): [string, string, number] => ["accent-text", bg, TEXT],
  ),
  ...surfaces.map((bg): [string, string, number] => ["border-control", bg, UI]),
  // The accent as an indicator: the fill of a checked box or switch, a selected border.
  ...surfaces.map((bg): [string, string, number] => ["accent", bg, UI]),
  ...["bg-canvas", "bg-surface", "bg-raised", "bg-sunken"].map((bg): [string, string, number] => [
    "focus",
    bg,
    UI,
  ]),
  // Initials on the fixed palette of the avatar (ADR 0082).
  ...AVATARS.map((bg): [string, string, number] => ["text", bg, TEXT]),
  ...["success", "warning", "danger", "proposed"].flatMap((status): [string, string, number][] => [
    [status, "bg-canvas", TEXT],
    [status, "bg-surface", TEXT],
    [status, "bg-raised", TEXT],
    [status, "bg-sunken", TEXT],
    [status, "bg-hover", TEXT],
    [status, `${status}-subtle`, TEXT],
    ["text", `${status}-subtle`, TEXT],
    ["text-muted", `${status}-subtle`, TEXT],
  ]),
];

describe.each([
  ["light", light],
  ["dark", dark],
])("contrast in the %s theme", (_name, theme) => {
  it.each(pairs)("%s on %s is at least %s:1", (foreground, background, minimum) => {
    const value = (token: string) => {
      const found = theme.get(`--color-${token}`) ?? light.get(`--color-${token}`);
      if (!found) {
        throw new Error(`--color-${token} is not defined`);
      }
      return found;
    };
    expect(ratio(value(foreground), value(background))).toBeGreaterThanOrEqual(minimum);
  });
});

describe("the dark theme", () => {
  it("has the same colors for the system preference and the explicit choice", () => {
    const colors = (tokens: Map<string, string>) =>
      [...tokens].filter(([name]) => name.startsWith("--")).sort(([x], [y]) => x.localeCompare(y));
    expect(colors(darkBySystem)).toEqual(colors(dark));
  });
});

describe("the comfortable density", () => {
  // ADR 0082 amends ADR 0022: a pointer target is at least 44 x 44 px with a coarse pointer.
  it.each([
    ["on request", block(':root[data-density="comfortable"]')],
    ["with a coarse pointer", block(':root:not([data-density="compact"])')],
  ])("makes buttons and inputs 44 px high %s", (_name, tokens) => {
    expect(tokens.get("--control-height-md")).toBe("44px");
    expect(tokens.get("--control-height-lg")).toBe("44px");
  });

  // The hit area of a chip reaches the target size, while the chip stays 20 px high.
  it.each([
    ["on request", block(':root[data-density="comfortable"]')],
    ["with a coarse pointer", block(':root:not([data-density="compact"])')],
  ])("gives a chip a 44 px hit area %s", (_name, tokens) => {
    expect(tokens.get("--size-hit-target")).toBe("44px");
  });
});

describe("the chip", () => {
  it("is 20 px high with a fine pointer", () => {
    expect(light.get("--size-chip-height")).toBe("20px");
    expect(light.get("--size-hit-target")).toBe("0px");
  });
});
