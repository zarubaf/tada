import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it } from "vitest";
import { TypeFilter } from "./TypeFilter";

const user = userEvent.setup({ delay: null });

const TYPES = [
  { id: "all", label: "Alle", count: 24 },
  { id: "tasks", label: "Aufgaben", count: 15 },
  { id: "commitments", label: "Zusagen" },
];

function Harness() {
  const [value, setValue] = useState("all");
  return (
    <>
      <TypeFilter label="Typ" options={TYPES} value={value} onChange={setValue} />
      <p data-testid="value">{value}</p>
    </>
  );
}

describe("TypeFilter", () => {
  it("is a group of toggle buttons with the counts, and no tabs", () => {
    render(<Harness />);

    expect(screen.getByRole("radiogroup", { name: "Typ" })).toBeInTheDocument();
    expect(screen.queryByRole("tab")).not.toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "Alle 24" })).toBeChecked();
    expect(screen.getByRole("radio", { name: "Aufgaben 15" })).not.toBeChecked();
  });

  it("shows an icon before the label without changing the name", () => {
    render(
      <TypeFilter
        label="Typ"
        options={[
          { id: "a", label: "Aufgaben", icon: <svg data-testid="icon" aria-hidden="true" /> },
        ]}
        value="a"
        onChange={() => {}}
      />,
    );
    expect(screen.getByRole("radio", { name: "Aufgaben" })).toContainElement(
      screen.getByTestId("icon"),
    );
  });

  it("reports the choice", async () => {
    render(<Harness />);

    await user.click(screen.getByRole("radio", { name: "Zusagen" }));

    expect(screen.getByTestId("value")).toHaveTextContent("commitments");
  });

  it("keeps one choice when the chosen option is pressed again", async () => {
    render(<Harness />);

    await user.click(screen.getByRole("radio", { name: "Alle 24" }));

    expect(screen.getByTestId("value")).toHaveTextContent("all");
    expect(screen.getByRole("radio", { name: "Alle 24" })).toBeChecked();
  });

  it("has one tab stop and moves with the arrow keys", async () => {
    render(<Harness />);

    await user.tab();
    expect(screen.getByRole("radio", { name: "Alle 24" })).toHaveFocus();
    await user.keyboard("{ArrowRight}");

    expect(screen.getByRole("radio", { name: "Aufgaben 15" })).toHaveFocus();
  });
});
