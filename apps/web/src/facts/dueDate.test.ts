import { describe, expect, it } from "vitest";
import { isOverdue, todayLocal } from "./dueDate";

describe("isOverdue", () => {
  it("is true only before today", () => {
    expect(isOverdue("2030-05-15", "2030-05-16")).toBe(true);
    expect(isOverdue("2030-05-16", "2030-05-16")).toBe(false);
    expect(isOverdue(null, "2030-05-16")).toBe(false);
  });
});

describe("todayLocal", () => {
  it("is the civil date of the browser time zone", () => {
    expect(todayLocal(new Date(2030, 4, 6, 23, 59))).toBe("2030-05-06");
  });
});
