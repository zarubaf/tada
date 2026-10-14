import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { LEVEL_ICONS } from "./LevelIcon";

/** The number of full bars: the sign of a level that needs no color. */
function fullBars(level: keyof typeof LEVEL_ICONS) {
  const Icon = LEVEL_ICONS[level];
  const { container } = render(<Icon size={14} />);
  return [...container.querySelectorAll("rect")].filter(
    (bar) => bar.getAttribute("opacity") === "1",
  ).length;
}

describe("LevelIcon", () => {
  it("fills one, two and three of three bars for low, medium and high", () => {
    expect([fullBars("low"), fullBars("medium"), fullBars("high")]).toEqual([1, 2, 3]);
  });

  it("is a decoration", () => {
    const Icon = LEVEL_ICONS.high;
    const { container } = render(<Icon />);
    expect(container.querySelector("svg")).toHaveAttribute("aria-hidden", "true");
  });
});
