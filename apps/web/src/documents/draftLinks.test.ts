import { describe, expect, it } from "vitest";
import type { LinkTarget } from "../api/client";
import { sourceNumbers } from "./draftLinks";

const A = "tada:source/0198f5a6-7c1e-7000-8000-000000000001#0-5";
const B = "tada:source/0198f5a6-7c1e-7000-8000-000000000002#10-20";
const FACT = "tada:fact/0198f5a6-7c1e-7000-8000-0000000000f1?v=1";

const source = (id: string): LinkTarget => ({
  kind: "source",
  source_version_id: id,
  passage: { start: 0, end: 5, quote: "Hallo" },
});

describe("sourceNumbers", () => {
  it("numbers each source by its first appearance", () => {
    const markdown = `Eins [a](${B}).\nZwei [b](${A}).\nDrei [c](${B}).`;
    const numbers = sourceNumbers(markdown, { [A]: source("a"), [B]: source("b") });
    expect([...numbers]).toEqual([
      [B, 1],
      [A, 2],
    ]);
  });

  it("skips a source that the reader cannot see and each fact link", () => {
    const markdown = `[a](${A}) [](${FACT}) [b](${B})`;
    const numbers = sourceNumbers(markdown, { [A]: { kind: "hidden" }, [B]: source("b") });
    expect([...numbers]).toEqual([[B, 1]]);
  });
});
