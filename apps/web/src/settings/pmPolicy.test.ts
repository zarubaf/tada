import { describe, expect, it } from "vitest";
import type { PmPolicy } from "../api/client";
import { formOf, timeOf, valuesOf, valuesWith } from "./pmPolicy";

const policy: PmPolicy = {
  local_time: "08:30:00",
  time_zone: "Europe/Zurich",
  weekdays: 0b001_1111,
  quiet_start: "20:00:00",
  quiet_end: "08:00:00",
  cooldown_hours: 20,
  follow_up_working_days: 2,
  look_ahead_working_days: 2,
  paused: false,
  ai_enabled: false,
  version: 4,
};

describe("pmPolicy", () => {
  it("shows the times without seconds", () => {
    expect(formOf(policy)).toMatchObject({ local_time: "08:30", quiet_end: "08:00" });
  });

  it("reads a time with one or two hour digits, with a colon or a point", () => {
    expect(timeOf("8:30")).toBe("08:30:00");
    expect(timeOf(" 20.15 ")).toBe("20:15:00");
    expect(timeOf("24:00")).toBeUndefined();
    expect(timeOf("8:3")).toBeUndefined();
    expect(timeOf("acht")).toBeUndefined();
  });

  it("sends the saved form unchanged, with the switches of the saved policy", () => {
    const { version: _version, ...values } = policy;
    expect(valuesOf(formOf(policy), { paused: true, ai_enabled: true })).toEqual({
      values: { ...values, paused: true, ai_enabled: true },
    });
  });

  it("accepts the ends of each range", () => {
    const form = {
      ...formOf(policy),
      cooldown_hours: "168",
      follow_up_working_days: "1",
      look_ahead_working_days: "0",
    };
    expect(valuesOf(form, policy)).toHaveProperty("values.look_ahead_working_days", 0);
    expect(
      valuesOf({ ...form, cooldown_hours: "1", follow_up_working_days: "10" }, policy),
    ).toHaveProperty("values");
  });

  it("names each field outside its range", () => {
    const form = {
      ...formOf(policy),
      local_time: "25:00",
      weekdays: 0,
      time_zone: "",
      quiet_start: "",
      cooldown_hours: "169",
      follow_up_working_days: "0",
      look_ahead_working_days: "11",
    };
    expect(valuesOf(form, policy)).toEqual({
      errors: {
        local_time: true,
        weekdays: true,
        time_zone: true,
        quiet_start: true,
        cooldown_hours: true,
        follow_up_working_days: true,
        look_ahead_working_days: true,
      },
    });
  });

  it("refuses a number with a fraction or a sign", () => {
    expect(valuesOf({ ...formOf(policy), cooldown_hours: "2.5" }, policy)).toEqual({
      errors: { cooldown_hours: true },
    });
    expect(valuesOf({ ...formOf(policy), cooldown_hours: "-1" }, policy)).toEqual({
      errors: { cooldown_hours: true },
    });
  });

  it("changes one switch of a saved policy", () => {
    expect(valuesWith(policy, { paused: true })).toMatchObject({
      paused: true,
      cooldown_hours: 20,
    });
    expect(valuesWith(policy, { paused: true })).not.toHaveProperty("version");
  });
});
