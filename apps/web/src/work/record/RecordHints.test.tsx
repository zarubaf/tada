import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { RecordHints } from "./RecordHints";

describe("RecordHints", () => {
  it("lists the hints under the heading Hinweise", () => {
    render(<RecordHints hints={["Datum überschritten", "ACT-002 ist erst danach fällig"]} />);
    const section = screen.getByRole("region", { name: "Hinweise" });
    expect(
      within(section)
        .getAllByRole("listitem")
        .map((item) => item.textContent),
    ).toEqual(["Datum überschritten", "ACT-002 ist erst danach fällig"]);
    expect(within(section).getByRole("heading", { level: 3, name: "Hinweise" })).toBeVisible();
  });

  it("shows nothing without a hint", () => {
    const { container } = render(<RecordHints hints={[]} />);
    expect(container).toBeEmptyDOMElement();
  });
});
