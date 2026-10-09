import { describe, expect, it } from "vitest";
import { authorKindName } from "./authorKind";

describe("authorKindName", () => {
  it("names the known kinds", () => {
    expect(authorKindName("ai")).toBe("KI-Client");
    expect(authorKindName("service")).toBe("Dienst");
  });

  it("falls back to a generic label for a kind that a newer server adds", () => {
    expect(authorKindName("robot")).toBe("Unbekannter Absender");
  });
});
