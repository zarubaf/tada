import { describe, expect, it } from "vitest";
import { uuidv7 } from "./uuid";

describe("uuidv7", () => {
  it("has the version 7, the variant 10 and the layout of a UUID", () => {
    expect(uuidv7()).toMatch(
      /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/,
    );
  });

  it("starts with the time in milliseconds", () => {
    const at = Date.UTC(2030, 4, 18, 8, 0, 0);
    const hex = uuidv7(at).replaceAll("-", "").slice(0, 12);
    expect(Number.parseInt(hex, 16)).toBe(at);
  });

  it("differs between calls in the same millisecond", () => {
    expect(uuidv7(1)).not.toBe(uuidv7(1));
  });
});
