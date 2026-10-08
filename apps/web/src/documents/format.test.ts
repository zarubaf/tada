import { describe, expect, it } from "vitest";
import { formatSize, hashPrefix, mediaTypeKind } from "./format";

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

describe("mediaTypeKind", () => {
  it.each([
    ["application/pdf", "pdf"],
    ["text/plain; charset=utf-8", "text"],
    ["text/markdown; charset=utf-8", "text"],
    ["text/csv", "text"],
    ["application/vnd.openxmlformats-officedocument.wordprocessingml.document", "office"],
    ["application/vnd.oasis.opendocument.spreadsheet", "office"],
    ["image/png", "image"],
    ["image/heic", "image"],
    ["application/zip", "other"],
    ["text/html", "other"],
  ])("maps %s to %s", (mediaType, kind) => {
    expect(mediaTypeKind(mediaType)).toBe(kind);
  });
});
