import { describe, expect, it } from "vitest";
import { actionStatusChoices, canMakeFirm, commitmentStatusChoices } from "./status";

describe("status rules", () => {
  it("offers a done action only the reopen", () => {
    expect(actionStatusChoices("done")).toEqual(["done", "open"]);
  });

  it("offers a canceled action nothing", () => {
    expect(actionStatusChoices("canceled")).toEqual(["canceled"]);
  });

  it("never offers firm as a status choice", () => {
    expect(commitmentStatusChoices("conditional")).not.toContain("firm");
    expect(commitmentStatusChoices("firm")).toEqual(["firm", "fulfilled", "broken", "withdrawn"]);
  });

  it("makes only a conditional commitment firm", () => {
    expect(canMakeFirm("conditional")).toBe(true);
    expect(canMakeFirm("firm")).toBe(false);
    expect(canMakeFirm("broken")).toBe(false);
  });
});
