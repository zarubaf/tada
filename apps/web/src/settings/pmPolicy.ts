// The rules of the AI-PM form that need no React: the texts of the fields, the local checks and the
// request. The server checks the same ranges; these checks only give a fast answer.
import type { PmPolicy } from "../api/client";

/** The fields of the form, in the order of the page. */
export const POLICY_FIELDS = [
  "local_time",
  "weekdays",
  "time_zone",
  "quiet_start",
  "quiet_end",
  "cooldown_hours",
  "follow_up_working_days",
  "look_ahead_working_days",
] as const;

export type PolicyField = (typeof POLICY_FIELDS)[number];

/** The fields that the member types as text. */
type TextField = Exclude<PolicyField, "weekdays" | "time_zone">;

export interface PolicyForm extends Record<TextField, string> {
  /** Bit 0 is Monday, bit 6 is Sunday, as in the API. */
  weekdays: number;
  /** The IANA zone that the member chose from the list. */
  time_zone: string;
}

/** The error of each field that failed a check. The page names the message. */
export type PolicyErrors = Partial<Record<PolicyField, true>>;

/** The ranges of the policy (doc/settings.md, C1). */
export const RANGES = {
  cooldown_hours: [1, 168],
  follow_up_working_days: [1, 10],
  look_ahead_working_days: [0, 10],
} as const;

/** All days of the week as bits. */
export const ALL_DAYS = 0b111_1111;

/** `08:30:00` as `08:30`: the seconds are always zero in tada. */
const hoursAndMinutes = (time: string) => time.slice(0, 5);

export function formOf(policy: PmPolicy): PolicyForm {
  return {
    local_time: hoursAndMinutes(policy.local_time),
    weekdays: policy.weekdays,
    time_zone: policy.time_zone,
    quiet_start: hoursAndMinutes(policy.quiet_start),
    quiet_end: hoursAndMinutes(policy.quiet_end),
    cooldown_hours: policy.cooldown_hours.toString(),
    follow_up_working_days: policy.follow_up_working_days.toString(),
    look_ahead_working_days: policy.look_ahead_working_days.toString(),
  };
}

/** `8:30`, `08:30` or `8.30` as `08:30:00`; anything else is undefined. */
export function timeOf(text: string): string | undefined {
  const match = /^([01]?\d|2[0-3])[:.]([0-5]\d)$/.exec(text.trim());
  return match ? `${match[1]?.padStart(2, "0")}:${match[2]}:00` : undefined;
}

function wholeNumberIn(text: string, [min, max]: readonly [number, number]): number | undefined {
  const trimmed = text.trim();
  if (!/^\d+$/.test(trimmed)) {
    return undefined;
  }
  const value = Number(trimmed);
  return value >= min && value <= max ? value : undefined;
}

/** The values of a policy without its version: what the form sends with `expected_version`. */
export type PolicyValues = Omit<PmPolicy, "version">;

/**
 * The values of the form, or the fields that fail a check. `paused` and `ai_enabled` are not in the
 * form: they have their own switches and come from the saved policy.
 */
export function valuesOf(
  form: PolicyForm,
  saved: Pick<PmPolicy, "paused" | "ai_enabled">,
): { values: PolicyValues } | { errors: PolicyErrors } {
  const errors: PolicyErrors = {};
  const time = (field: "local_time" | "quiet_start" | "quiet_end") => {
    const value = timeOf(form[field]);
    if (value === undefined) {
      errors[field] = true;
    }
    return value ?? "";
  };
  const count = (field: keyof typeof RANGES) => {
    const value = wholeNumberIn(form[field], RANGES[field]);
    if (value === undefined) {
      errors[field] = true;
    }
    return value ?? 0;
  };
  const values: PolicyValues = {
    local_time: time("local_time"),
    weekdays: form.weekdays,
    time_zone: form.time_zone,
    quiet_start: time("quiet_start"),
    quiet_end: time("quiet_end"),
    cooldown_hours: count("cooldown_hours"),
    follow_up_working_days: count("follow_up_working_days"),
    look_ahead_working_days: count("look_ahead_working_days"),
    paused: saved.paused,
    ai_enabled: saved.ai_enabled,
  };
  if ((form.weekdays & ALL_DAYS) === 0) {
    errors.weekdays = true;
  }
  if (form.time_zone === "") {
    errors.time_zone = true;
  }
  return Object.keys(errors).length > 0 ? { errors } : { values };
}

/** The values of a saved policy, with one switch changed. */
export function valuesWith(policy: PmPolicy, change: Partial<PolicyValues>): PolicyValues {
  const { version: _version, ...values } = policy;
  return { ...values, ...change };
}

/** The bit of a day: 0 is Monday. */
export const dayBit = (day: number) => 1 << day;
