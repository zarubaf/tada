import { describe, expect, it } from "vitest";
import { differs, footerText } from "./version";

describe("version", () => {
  it("writes the footer as version and short commit", () => {
    expect(footerText({ version: "0.3.0", commit: "1a2b3c4" })).toBe("tada 0.3.0 · 1a2b3c4");
  });

  it.each([
    ["the same build", { version: "0.3.0", commit: "1a2b3c4" }, false],
    ["another version", { version: "0.4.0", commit: "1a2b3c4" }, true],
    ["another commit", { version: "0.3.0", commit: "9f8e7d6" }, true],
    ["an unknown commit", { version: "0.3.0", commit: "unknown" }, false],
    ["an unknown version", { version: "unknown", commit: "unknown" }, true],
  ])("compares with %s", (_, other, expected) => {
    expect(differs({ version: "0.3.0", commit: "1a2b3c4" }, other)).toBe(expected);
  });
});
