// How the web client shows a fact value. Every page formats values through this module, so that
// a date, an amount and an unknown value look the same everywhere (ADR 0049).
//
// The server returns a value as JSON with a `type` tag and the value type of its field. Dates are
// civil dates without a time zone; only a time (see `formatDateTime`) needs one.

import type { Label, ValueType } from "../api/client";
import { hasMessage, LOCALE, t } from "../i18n";

const EN_DASH = "–";

/** The value of a fact or a proposal as the API returns it. The state `unknown` has none. */
export interface ValueInput {
  value?: unknown;
  approximate?: boolean | null | undefined;
}

type Granularity = "day" | "week" | "month";

type FactValue =
  | { type: "text"; text: string }
  | { type: "boolean"; value: boolean }
  | { type: "quantity"; min: string; max: string }
  | { type: "money"; min: number; max: number }
  | { type: "date"; date: string }
  | { type: "date-window"; start: string; end: string; granularity: Granularity }
  | { type: "choice"; keys: string[] }
  | { type: "reference"; target: string; id: string };

const dayFormat = new Intl.DateTimeFormat(LOCALE, {
  day: "2-digit",
  month: "2-digit",
  year: "numeric",
  timeZone: "UTC",
});
const monthFormat = new Intl.DateTimeFormat(LOCALE, {
  month: "long",
  year: "numeric",
  timeZone: "UTC",
});
const number = new Intl.NumberFormat(LOCALE, { maximumFractionDigits: 6 });

/** The value could not be shown: an invalid date, a time zone that `Intl` does not know. */
class Unshowable extends Error {}

/** A civil date (`2030-05-18`) as a `Date` at midnight UTC. */
function civilDate(date: string): Date {
  const parsed = new Date(`${date}T00:00:00Z`);
  if (Number.isNaN(parsed.getTime())) {
    throw new Unshowable(date);
  }
  return parsed;
}

/** A civil date (`2030-05-18`) as 18.05.2030. */
function formatDate(date: string): string {
  return dayFormat.format(civilDate(date));
}

/** A range as „from – to“, or one value when both ends read the same. */
function range(from: string, to: string): string {
  return from === to ? from : `${from} ${EN_DASH} ${to}`;
}

function formatQuantity(text: string): string {
  const parsed = Number(text);
  return Number.isNaN(parsed) ? text : number.format(parsed);
}

/** The label of a unit for `count`, which picks the singular or the plural. */
function unitLabel(unit: string, count: number): string {
  const id = `unit-${unit}`;
  return hasMessage(id) ? t(id, { count }) : unit;
}

function money(minor: number, currency: string | undefined): string {
  const amount = minor / 100;
  return currency
    ? new Intl.NumberFormat(LOCALE, { style: "currency", currency }).format(amount)
    : new Intl.NumberFormat(LOCALE, { minimumFractionDigits: 2 }).format(amount);
}

function formatWindow(start: string, end: string, granularity: Granularity): string {
  if (granularity === "month") {
    const format = (date: string) => monthFormat.format(civilDate(date));
    return range(format(start), format(end));
  }
  // A week reads as its first and last day.
  return range(formatDate(start), formatDate(end));
}

function choiceLabel(key: string, type: ValueType | undefined): string {
  const choice =
    type?.type === "choice" ? type.values.find((value) => value.key === key) : undefined;
  return choice ? formatLabel(choice.label) : key;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

/** The value as a typed value, or undefined when it has no known shape. */
function parse(value: unknown): FactValue | undefined {
  if (!isRecord(value)) {
    return undefined;
  }
  const { type } = value;
  const isText = (key: string) => typeof value[key] === "string";
  const isNumber = (key: string) => typeof value[key] === "number";
  switch (type) {
    case "text":
      return isText("text") ? (value as FactValue) : undefined;
    case "boolean":
      return typeof value.value === "boolean" ? (value as FactValue) : undefined;
    case "quantity":
      return isText("min") && isText("max") ? (value as FactValue) : undefined;
    case "money":
      return isNumber("min") && isNumber("max") ? (value as FactValue) : undefined;
    case "date":
      return isText("date") ? (value as FactValue) : undefined;
    case "date-window":
      return isText("start") && isText("end") && isText("granularity")
        ? (value as FactValue)
        : undefined;
    case "choice":
      return Array.isArray(value.keys) ? (value as FactValue) : undefined;
    case "reference":
      return isText("target") && isText("id") ? (value as FactValue) : undefined;
    default:
      return undefined;
  }
}

function formatTyped(value: FactValue, type: ValueType | undefined): string {
  switch (value.type) {
    case "text":
      return value.text;
    case "boolean":
      return t(value.value ? "value-yes" : "value-no");
    case "quantity": {
      const amount = range(formatQuantity(value.min), formatQuantity(value.max));
      return type?.type === "quantity"
        ? `${amount} ${unitLabel(type.unit, Number(value.max))}`
        : amount;
    }
    case "money": {
      const currency = type?.type === "money" ? type.currency : undefined;
      return range(money(value.min, currency), money(value.max, currency));
    }
    case "date":
      return formatDate(value.date);
    case "date-window":
      return formatWindow(value.start, value.end, value.granularity);
    case "choice":
      return value.keys.map((key) => choiceLabel(key, type)).join(", ");
    case "reference": {
      const id = `value-reference-${value.target}`;
      return hasMessage(id) ? t(id) : t("value-unsupported");
    }
  }
}

/**
 * The text of a fact value. An absent value reads „Unbekannt“, never an empty string. A value of an
 * unknown shape reads „Nicht darstellbar“. `type` is the value type of the field; without it
 * the unit, the currency and the labels of choices are missing.
 */
export function formatValue(input: ValueInput, type: ValueType | undefined): string {
  if (input.value === undefined || input.value === null) {
    return t("knowledge-unknown");
  }
  const value = parse(input.value);
  if (!value) {
    return t("value-unsupported");
  }
  try {
    const text = formatTyped(value, type);
    return input.approximate ? t("value-approximate", { value: text }) : text;
  } catch (error) {
    if (error instanceof Unshowable) {
      return t("value-unsupported");
    }
    throw error;
  }
}

/** The text of a field label or a choice label: a Fluent message or the text of the event. */
export function formatLabel(label: Label): string {
  return label.kind === "message" ? t(label.id) : label.text;
}

/** A time as 03.10.2026, 14:12 in `timeZone`, the IANA zone of the event. */
export function formatDateTime(iso: string, timeZone: string): string {
  try {
    return new Intl.DateTimeFormat(LOCALE, {
      day: "2-digit",
      month: "2-digit",
      year: "numeric",
      hour: "2-digit",
      minute: "2-digit",
      timeZone,
    }).format(new Date(iso));
  } catch {
    // `Intl` throws a RangeError for an invalid time or an unknown time zone.
    return t("value-unsupported");
  }
}
