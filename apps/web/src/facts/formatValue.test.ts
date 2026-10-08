import { describe, expect, it } from "vitest";
import type { ValueType } from "../api/client";
import { formatDateTime, formatLabel, formatValue } from "./formatValue";

const EN_DASH = "–";
const text: ValueType = { type: "text" };
const quantity: ValueType = { type: "quantity", unit: "person_per_day" };
const money: ValueType = { type: "money", currency: "CHF" };
const window: ValueType = { type: "date-window", granularity: null };
const choice: ValueType = {
  type: "choice",
  multiple: true,
  values: [
    { key: "airshow", label: { kind: "message", id: "field-components-airshow" } },
    { key: "catering", label: { kind: "text", text: "Festwirtschaft" } },
  ],
};

describe("formatValue", () => {
  it("shows Unbekannt for a value that is absent", () => {
    expect(formatValue({}, text)).toBe("Unbekannt");
    expect(formatValue({ value: null }, quantity)).toBe("Unbekannt");
  });

  it("shows text as it is", () => {
    expect(formatValue({ value: { type: "text", text: "Flugplatz Testwil" } }, text)).toBe(
      "Flugplatz Testwil",
    );
  });

  it("shows a boolean as Ja or Nein", () => {
    const type: ValueType = { type: "boolean" };
    expect(formatValue({ value: { type: "boolean", value: true } }, type)).toBe("Ja");
    expect(formatValue({ value: { type: "boolean", value: false } }, type)).toBe("Nein");
  });

  it("shows a quantity with the unit label and a range with an en dash", () => {
    const one = { type: "quantity", min: "20000", max: "20000" };
    expect(formatValue({ value: one }, quantity)).toMatch(/^20.000 Personen pro Tag$/);
    const range = { type: "quantity", min: "1.5", max: "2" };
    expect(formatValue({ value: range }, quantity)).toBe(`1.5 ${EN_DASH} 2 Personen pro Tag`);
  });

  it("shows the key of a unit without a label", () => {
    const type: ValueType = { type: "quantity", unit: "kilo_gram" };
    expect(formatValue({ value: { type: "quantity", min: "3", max: "3" } }, type)).toBe(
      "3 kilo_gram",
    );
  });

  it("shows money in CHF from the minor unit", () => {
    expect(formatValue({ value: { type: "money", min: 1500, max: 1500 } }, money)).toBe(
      "CHF\u00a015.00",
    );
    expect(formatValue({ value: { type: "money", min: 1000, max: 2550 } }, money)).toBe(
      `CHF\u00a010.00 ${EN_DASH} CHF\u00a025.50`,
    );
  });

  it("shows a date as dd.mm.yyyy", () => {
    expect(formatValue({ value: { type: "date", date: "2026-10-03" } }, { type: "date" })).toBe(
      "03.10.2026",
    );
  });

  it("shows a date window by its granularity", () => {
    const days = {
      type: "date-window",
      start: "2030-05-18",
      end: "2030-05-19",
      granularity: "day",
    };
    expect(formatValue({ value: days }, window)).toBe(`18.05.2030 ${EN_DASH} 19.05.2030`);
    expect(formatValue({ value: { ...days, end: "2030-05-18" } }, window)).toBe("18.05.2030");
    const months = {
      type: "date-window",
      start: "2030-05-01",
      end: "2030-06-30",
      granularity: "month",
    };
    expect(formatValue({ value: months }, window)).toBe(`Mai 2030 ${EN_DASH} Juni 2030`);
    expect(formatValue({ value: { ...months, end: "2030-05-31" } }, window)).toBe("Mai 2030");
  });

  it("shows the labels of the chosen keys", () => {
    const value = { type: "choice", keys: ["airshow", "catering"] };
    expect(formatValue({ value }, choice)).toBe("Flugshow, Festwirtschaft");
  });

  it("marks an approximate value", () => {
    const value = { type: "quantity", min: "20000", max: "20000" };
    expect(formatValue({ value, approximate: true }, quantity)).toMatch(/^ca\. 20.000 Personen/);
  });

  it("shows a negative amount, an approximate range and money without a currency", () => {
    expect(formatValue({ value: { type: "money", min: -250, max: -250 } }, money)).toMatch(
      /^-CHF.2\.50$|^CHF-2\.50$/,
    );
    const range = { type: "quantity", min: "10", max: "20" };
    expect(formatValue({ value: range, approximate: true }, quantity)).toBe(
      `ca. 10 ${EN_DASH} 20 Personen pro Tag`,
    );
    expect(formatValue({ value: { type: "money", min: 1500, max: 1500 } }, undefined)).toBe(
      "15.00",
    );
  });

  it("shows a week window by its first and last day", () => {
    const week = {
      type: "date-window",
      start: "2030-05-13",
      end: "2030-05-19",
      granularity: "week",
    };
    expect(formatValue({ value: week }, window)).toBe(`13.05.2030 ${EN_DASH} 19.05.2030`);
  });

  it("uses the singular of a unit for one", () => {
    const type: ValueType = { type: "quantity", unit: "day" };
    expect(formatValue({ value: { type: "quantity", min: "1", max: "1" } }, type)).toBe("1 Tag");
    expect(formatValue({ value: { type: "quantity", min: "3", max: "3" } }, type)).toBe("3 Tage");
  });

  it("shows Nicht darstellbar for an invalid date", () => {
    expect(formatValue({ value: { type: "date", date: "2030-13-45" } }, { type: "date" })).toBe(
      "Nicht darstellbar",
    );
    const bad = { type: "date-window", start: "x", end: "y", granularity: "month" };
    expect(formatValue({ value: bad }, window)).toBe("Nicht darstellbar");
  });

  it("does not leave a value that it cannot show empty", () => {
    expect(formatValue({ value: { type: "colour" } }, text)).toBe("Nicht darstellbar");
    expect(formatValue({ value: 7 }, undefined)).toBe("Nicht darstellbar");
  });
});

describe("formatLabel", () => {
  it("resolves a message and keeps a text", () => {
    expect(formatLabel({ kind: "message", id: "field-venue" })).toBe("Ort");
    expect(formatLabel({ kind: "text", text: "Hangar" })).toBe("Hangar");
  });
});

describe("formatDateTime", () => {
  it("shows the time in the time zone of the event", () => {
    expect(formatDateTime("2026-10-03T12:12:00Z", "Europe/Zurich")).toBe("03.10.2026, 14:12");
  });

  it("shows Nicht darstellbar for an invalid time zone or time", () => {
    expect(formatDateTime("2026-10-03T12:12:00Z", "Mars/Olympus")).toBe("Nicht darstellbar");
    expect(formatDateTime("not a time", "Europe/Zurich")).toBe("Nicht darstellbar");
  });
});
