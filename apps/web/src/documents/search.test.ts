import { describe, expect, it } from "vitest";
import { searchParam } from "./search";

describe("searchParam", () => {
  it("trims the text", () => {
    expect(searchParam("  Programm  ")).toBe("Programm");
  });

  it("sends no filter for empty text or white space", () => {
    expect(searchParam("")).toBeUndefined();
    expect(searchParam("   ")).toBeUndefined();
  });

  it("keeps umlauts and inner spaces", () => {
    expect(searchParam("Flugtag Übersicht")).toBe("Flugtag Übersicht");
  });
});
