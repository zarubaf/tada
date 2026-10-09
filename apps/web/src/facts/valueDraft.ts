// What a reviewer types when they edit a proposed value, and how it becomes a typed value. The form
// (`ValueInput`) only shows the draft; this module owns the rules (ADR 0049, ADR 0050).
//
// A draft holds text, because a field keeps what the member typed until it is valid.

import type { FactValue, ValueType } from "../api/client";
import { t } from "../i18n";

export type Granularity = "day" | "week" | "month";

export interface Draft {
  text: string;
  /** A boolean: "yes", "no" or "" while nobody chose. */
  flag: "yes" | "no" | "";
  /** The minimum of a quantity or an amount, in major units (15.00) for money. */
  min: string;
  /** The maximum. Empty means the same as the minimum. */
  max: string;
  date: string;
  start: string;
  end: string;
  granularity: Granularity | "";
  keys: string[];
  approximate: boolean;
}

/** The field of the draft that an error belongs to. */
export type DraftField =
  | "text"
  | "flag"
  | "min"
  | "max"
  | "date"
  | "start"
  | "end"
  | "granularity"
  | "keys";

export type DraftResult =
  | { ok: true; value: FactValue }
  | { ok: false; errors: Partial<Record<DraftField, string>> };

export function emptyDraft(): Draft {
  return {
    text: "",
    flag: "",
    min: "",
    max: "",
    date: "",
    start: "",
    end: "",
    granularity: "",
    keys: [],
    approximate: false,
  };
}

const EDITABLE = new Set<string>([
  "text",
  "boolean",
  "quantity",
  "money",
  "date",
  "date-window",
  "choice",
]);

/**
 * True for a value type that the reviewer can edit. A reference points to a record. The list of
 * value types is open: a type that this client does not know is not editable either.
 */
export function isEditable(type: ValueType): boolean {
  return EDITABLE.has(type.type);
}

const DECIMAL = /^-?\d+([.,]\d+)?$/;
const AMOUNT = /^(\d+)(?:[.,](\d{1,2}))?$/;
const CIVIL_DATE = /^(\d{4})-(\d{2})-(\d{2})$/;

function isCivilDate(text: string): boolean {
  const match = CIVIL_DATE.exec(text);
  if (!match) {
    return false;
  }
  const [year, month, day] = match.slice(1).map(Number) as [number, number, number];
  const parsed = new Date(Date.UTC(year, month - 1, day));
  return (
    parsed.getUTCFullYear() === year &&
    parsed.getUTCMonth() === month - 1 &&
    parsed.getUTCDate() === day
  );
}

/** An amount like 15.1 or 0,29 as minor units (1510, 29), or undefined. No float is involved. */
function minorUnits(text: string): number | undefined {
  const match = AMOUNT.exec(text.trim());
  if (!match) {
    return undefined;
  }
  const [, whole = "0", cents = ""] = match;
  return Number(whole) * 100 + Number(cents.padEnd(2, "0"));
}

function quantityResult(draft: Draft): DraftResult {
  const min = draft.min.trim();
  const max = draft.max.trim() === "" ? min : draft.max.trim();
  const errors: Partial<Record<DraftField, string>> = {};
  if (min === "") {
    errors.min = t("value-error-required");
  } else if (!DECIMAL.test(min)) {
    errors.min = t("value-error-number");
  }
  if (!errors.min && !DECIMAL.test(max)) {
    errors.max = t("value-error-number");
  }
  const [low, high] = [min, max].map((text) => Number(text.replace(",", ".")));
  if (!errors.min && !errors.max && (low ?? 0) > (high ?? 0)) {
    errors.max = t("value-error-range");
  }
  if (Object.keys(errors).length > 0) {
    return { ok: false, errors };
  }
  return {
    ok: true,
    value: { type: "quantity", min: min.replace(",", "."), max: max.replace(",", ".") },
  };
}

function moneyResult(draft: Draft): DraftResult {
  const min = minorUnits(draft.min);
  const max = draft.max.trim() === "" ? min : minorUnits(draft.max);
  const errors: Partial<Record<DraftField, string>> = {};
  if (draft.min.trim() === "") {
    errors.min = t("value-error-required");
  } else if (min === undefined) {
    errors.min = t("value-error-money");
  }
  if (!errors.min && max === undefined) {
    errors.max = t("value-error-money");
  }
  if (min !== undefined && max !== undefined && min > max) {
    errors.max = t("value-error-range");
  }
  if (min === undefined || max === undefined || Object.keys(errors).length > 0) {
    return { ok: false, errors };
  }
  return { ok: true, value: { type: "money", min, max } };
}

function windowResult(
  type: Extract<ValueType, { type: "date-window" }>,
  draft: Draft,
): DraftResult {
  const errors: Partial<Record<DraftField, string>> = {};
  const granularity = type.granularity ?? draft.granularity;
  if (!isCivilDate(draft.start)) {
    errors.start = draft.start === "" ? t("value-error-required") : t("value-error-date");
  }
  if (!isCivilDate(draft.end)) {
    errors.end = draft.end === "" ? t("value-error-required") : t("value-error-date");
  }
  if (!errors.start && !errors.end && draft.end < draft.start) {
    errors.end = t("value-error-window");
  }
  if (granularity === "" || granularity === undefined) {
    errors.granularity = t("value-error-granularity");
  }
  if (Object.keys(errors).length > 0 || !granularity) {
    return { ok: false, errors };
  }
  return {
    ok: true,
    value: { type: "date-window", start: draft.start, end: draft.end, granularity },
  };
}

/** The typed value of a draft for the value type of the field, or an error for each bad field. */
export function draftToValue(type: ValueType, draft: Draft): DraftResult {
  switch (type.type) {
    case "text":
      return draft.text.trim() === ""
        ? { ok: false, errors: { text: t("value-error-required") } }
        : { ok: true, value: { type: "text", text: draft.text.trim() } };
    case "boolean":
      return draft.flag === ""
        ? { ok: false, errors: { flag: t("value-error-required") } }
        : { ok: true, value: { type: "boolean", value: draft.flag === "yes" } };
    case "quantity":
      return quantityResult(draft);
    case "money":
      return moneyResult(draft);
    case "date":
      return isCivilDate(draft.date)
        ? { ok: true, value: { type: "date", date: draft.date } }
        : {
            ok: false,
            errors: {
              date: draft.date === "" ? t("value-error-required") : t("value-error-date"),
            },
          };
    case "date-window":
      return windowResult(type, draft);
    case "choice":
      if (draft.keys.length === 0) {
        return { ok: false, errors: { keys: t("value-error-choice") } };
      }
      if (!type.multiple && draft.keys.length > 1) {
        return { ok: false, errors: { keys: t("value-error-choice-single") } };
      }
      return { ok: true, value: { type: "choice", keys: draft.keys } };
    default:
      // A reference, or a value type of a newer server: no form makes a value for it.
      return { ok: false, errors: {} };
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function majorUnits(minor: unknown): string {
  return typeof minor === "number" ? (minor / 100).toFixed(2) : "";
}

/**
 * The draft that starts the edit: the proposed value. A value of another shape than the field
 * wants gives an empty draft, so the reviewer types a new one.
 */
export function initialDraft(type: ValueType, value: unknown, approximate = false): Draft {
  const draft = { ...emptyDraft(), approximate };
  if (!isRecord(value) || value.type !== type.type) {
    return draft;
  }
  const str = (key: string) => (typeof value[key] === "string" ? (value[key] as string) : "");
  switch (type.type) {
    case "text":
      return { ...draft, text: str("text") };
    case "boolean":
      return typeof value.value === "boolean"
        ? { ...draft, flag: value.value ? "yes" : "no" }
        : draft;
    case "quantity":
      return { ...draft, min: str("min"), max: str("max") === str("min") ? "" : str("max") };
    case "money":
      return {
        ...draft,
        min: majorUnits(value.min),
        max: value.max === value.min ? "" : majorUnits(value.max),
      };
    case "date":
      return { ...draft, date: str("date") };
    case "date-window": {
      const granularity = str("granularity");
      return {
        ...draft,
        start: str("start"),
        end: str("end"),
        granularity:
          granularity === "day" || granularity === "week" || granularity === "month"
            ? granularity
            : "",
      };
    }
    case "choice":
      return Array.isArray(value.keys)
        ? { ...draft, keys: value.keys.filter((key): key is string => typeof key === "string") }
        : draft;
    case "reference":
      return draft;
  }
}
