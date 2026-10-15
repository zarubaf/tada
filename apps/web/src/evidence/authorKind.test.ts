import { describe, expect, it } from "vitest";
import { authorKindName } from "./authorKind";

describe("authorKindName", () => {
  it("names the known kinds", () => {
    expect(authorKindName("ai")).toBe("KI-Programm");
    expect(authorKindName("service")).toBe("Automatik");
  });

  it("falls back to a generic label for a kind that a newer server adds", () => {
    expect(authorKindName("robot")).toBe("Unbekannter Absender");
  });
});
