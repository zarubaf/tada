import { describe, expect, it } from "vitest";
import type { ValueType } from "../api/client";
import { draftToValue, emptyDraft, initialDraft, isEditable } from "./valueDraft";

const text: ValueType = { type: "text" };
const quantity: ValueType = { type: "quantity", unit: "person_per_day" };
const money: ValueType = { type: "money", currency: "CHF" };
const windowType: ValueType = { type: "date-window", granularity: null };
const choice = (multiple: boolean): ValueType => ({
  type: "choice",
  multiple,
  values: [
    { key: "north", label: { kind: "text", text: "Nord" } },
    { key: "south", label: { kind: "text", text: "Süd" } },
  ],
});

describe("draftToValue", () => {
  it("reads a text", () => {
    const draft = { ...emptyDraft(), text: "Flugplatz Testwil" };
    expect(draftToValue(text, draft)).toEqual({
      ok: true,
      value: { type: "text", text: "Flugplatz Testwil" },
    });
  });

  it("rejects an empty text and names the field", () => {
    const result = draftToValue(text, emptyDraft());
    expect(result).toMatchObject({ ok: false, errors: { text: expect.any(String) } });
  });

  it("reads a quantity with a decimal comma and uses the minimum as the maximum", () => {
    const draft = { ...emptyDraft(), min: "12,5" };
    expect(draftToValue(quantity, draft)).toEqual({
      ok: true,
      value: { type: "quantity", min: "12.5", max: "12.5" },
    });
  });

  it("rejects a quantity that is not a number or whose range is reversed", () => {
    expect(draftToValue(quantity, { ...emptyDraft(), min: "viele" })).toMatchObject({
      ok: false,
      errors: { min: expect.any(String) },
    });
    expect(draftToValue(quantity, { ...emptyDraft(), min: "20", max: "10" })).toMatchObject({
      ok: false,
      errors: { max: expect.any(String) },
    });
  });

  it("converts an amount to minor units without float errors", () => {
    expect(draftToValue(money, { ...emptyDraft(), min: "15.10" })).toEqual({
      ok: true,
      value: { type: "money", min: 1510, max: 1510 },
    });
    expect(draftToValue(money, { ...emptyDraft(), min: "0,29", max: "1" })).toEqual({
      ok: true,
      value: { type: "money", min: 29, max: 100 },
    });
  });

  it("rejects an amount with more than two decimals", () => {
    expect(draftToValue(money, { ...emptyDraft(), min: "1.005" })).toMatchObject({
      ok: false,
      errors: { min: expect.any(String) },
    });
  });

  it("reads a date and rejects an impossible one", () => {
    const date: ValueType = { type: "date" };
    expect(draftToValue(date, { ...emptyDraft(), date: "2030-05-18" })).toEqual({
      ok: true,
      value: { type: "date", date: "2030-05-18" },
    });
    expect(draftToValue(date, { ...emptyDraft(), date: "2030-02-31" })).toMatchObject({
      ok: false,
      errors: { date: expect.any(String) },
    });
  });

  it("needs a granularity for a window without a fixed one", () => {
    const draft = { ...emptyDraft(), start: "2030-05-01", end: "2030-05-31" };
    expect(draftToValue(windowType, draft)).toMatchObject({
      ok: false,
      errors: { granularity: expect.any(String) },
    });
    expect(draftToValue(windowType, { ...draft, granularity: "month" })).toEqual({
      ok: true,
      value: { type: "date-window", start: "2030-05-01", end: "2030-05-31", granularity: "month" },
    });
  });

  it("uses the fixed granularity of the field", () => {
    const fixed: ValueType = { type: "date-window", granularity: "day" };
    const draft = { ...emptyDraft(), start: "2030-05-01", end: "2030-05-02" };
    expect(draftToValue(fixed, draft)).toMatchObject({
      ok: true,
      value: { granularity: "day" },
    });
  });

  it("rejects a window that ends before it starts", () => {
    const draft = {
      ...emptyDraft(),
      start: "2030-05-10",
      end: "2030-05-01",
      granularity: "day" as const,
    };
    expect(draftToValue(windowType, draft)).toMatchObject({
      ok: false,
      errors: { end: expect.any(String) },
    });
  });

  it("needs a key for a choice and only one for a single choice", () => {
    expect(draftToValue(choice(false), emptyDraft())).toMatchObject({
      ok: false,
      errors: { keys: expect.any(String) },
    });
    expect(
      draftToValue(choice(false), { ...emptyDraft(), keys: ["north", "south"] }),
    ).toMatchObject({ ok: false });
    expect(draftToValue(choice(true), { ...emptyDraft(), keys: ["north", "south"] })).toEqual({
      ok: true,
      value: { type: "choice", keys: ["north", "south"] },
    });
  });

  it("reads a boolean", () => {
    expect(draftToValue({ type: "boolean" }, { ...emptyDraft(), flag: "yes" })).toEqual({
      ok: true,
      value: { type: "boolean", value: true },
    });
  });
});

describe("initialDraft", () => {
  it("fills the draft from the proposed value", () => {
    expect(initialDraft(money, { type: "money", min: 1500, max: 2000 })).toMatchObject({
      min: "15.00",
      max: "20.00",
    });
    expect(initialDraft(text, { type: "text", text: "Hangar 3" })).toMatchObject({
      text: "Hangar 3",
    });
  });

  it("starts empty for a value of another shape", () => {
    expect(initialDraft(text, { type: "money", min: 1, max: 1 })).toEqual(emptyDraft());
    expect(initialDraft(text, undefined)).toEqual(emptyDraft());
  });
});

describe("isEditable", () => {
  it("excludes a reference", () => {
    expect(isEditable({ type: "reference", target: "event" })).toBe(false);
    expect(isEditable(text)).toBe(true);
  });

  it("excludes a value type that a newer server adds", () => {
    expect(isEditable({ type: "duration" } as unknown as ValueType)).toBe(false);
  });

  it("makes no value of a value type that it does not know", () => {
    const unknown = { type: "duration" } as unknown as ValueType;
    expect(draftToValue(unknown, emptyDraft())).toEqual({ ok: false, errors: {} });
  });
});
