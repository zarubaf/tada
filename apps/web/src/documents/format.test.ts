import { describe, expect, it } from "vitest";
import { formatSize, hashPrefix } from "./format";

describe("formatSize", () => {
  it("shows bytes below 1 KB", () => {
    expect(formatSize(0)).toBe("0 B");
    expect(formatSize(999)).toBe("999 B");
  });

  it("uses decimal units with one decimal place at most", () => {
    expect(formatSize(1_500)).toBe("1.5 KB");
    expect(formatSize(2_500_000)).toBe("2.5 MB");
    expect(formatSize(100_000_000)).toBe("100 MB");
  });
});

describe("hashPrefix", () => {
  it("shows the first 12 digits", () => {
    expect(hashPrefix("0123456789abcdef0123456789abcdef")).toBe("0123456789ab");
  });
});
