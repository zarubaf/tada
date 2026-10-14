import { IconAntennaBars1, IconAntennaBars5 } from "@tabler/icons-react";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { FieldSpec } from "./descriptor";
import { enumChoices } from "./enumChoices";

const spec: FieldSpec = {
  id: "impact",
  labelKey: "record-field-impact",
  control: "enum",
  pointer: "impact",
  read: () => "low",
  values: ["low", "medium", "high"],
  present: (value) => ({
    labelKey: `risk-level-${value}`,
    ...(value === "low" && { icon: IconAntennaBars1 }),
    ...(value === "high" && { icon: IconAntennaBars5 }),
  }),
};

describe("enumChoices", () => {
  it("offers the values, in order, with their German words", () => {
    const choices = enumChoices(spec);
    expect(choices.options.map(({ id, label }) => [id, label])).toEqual([
      ["low", "niedrig"],
      ["medium", "mittel"],
      ["high", "hoch"],
    ]);
    expect(choices.offered).toBe(true);
  });

  it("is not offered when only one value is left", () => {
    expect(enumChoices({ ...spec, values: ["low"] }).offered).toBe(false);
  });

  it("puts an icon before the word, as a decoration of the word and never as its only sign", () => {
    const [low, medium] = enumChoices(spec).options;
    render(
      <>
        <div data-testid="low">{low?.leading}</div>
        <div data-testid="medium">{medium?.leading}</div>
      </>,
    );
    const svg = screen.getByTestId("low").querySelector("svg");
    expect(svg).toHaveAttribute("aria-hidden", "true");
    // A value without an icon has no empty slot.
    expect(screen.getByTestId("medium")).toBeEmptyDOMElement();
  });
});
