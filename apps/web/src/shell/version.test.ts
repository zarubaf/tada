import { describe, expect, it } from "vitest";
import { CLIENT_BUILD_ID, footerText, outdated } from "./version";

describe("version", () => {
  it("writes the footer as version and short commit of the server", () => {
    expect(footerText({ product: "0.3.0", commit: "1a2b3c4" })).toBe("tada 0.3.0 · 1a2b3c4");
  });

  it("holds the fixed build ID of the tests", () => {
    expect(CLIENT_BUILD_ID).toBe("0123456789abcdef0123456789abcdef");
  });

  it.each([
    ["the same build", "0123456789abcdef0123456789abcdef", false],
    ["another build", "fedcba9876543210fedcba9876543210", true],
  ])("compares with %s", (_, web_build, expected) => {
    expect(outdated({ web_build })).toBe(expected);
  });
});
