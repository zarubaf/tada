import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { Select } from "./Select";

const options = [
  { id: "a", label: "Alle Termine" },
  { id: "b", label: "Überfällig" },
];

describe("Select", () => {
  it("picks an option and names the select by its label", async () => {
    const onChange = vi.fn();
    render(<Select label="Fällig" options={options} value="a" onChange={onChange} />);
    await userEvent.click(screen.getByRole("button", { name: /Fällig/ }));
    await userEvent.click(screen.getByRole("option", { name: "Überfällig" }));
    expect(onChange).toHaveBeenCalledWith("b");
  });

  it("looks like a toolbar control with a chevron icon, not the text caret of a field", () => {
    const { container } = render(
      <Select
        label="Fällig"
        labelHidden
        variant="toolbar"
        options={options}
        value="a"
        onChange={() => {}}
      />,
    );
    const trigger = screen.getByRole("button", { name: /Fällig/ });
    expect(trigger).toHaveTextContent("Alle Termine");
    expect(trigger.querySelector("svg")).not.toBeNull();
    expect(container).not.toHaveTextContent("▾");
  });

  it("keeps the text caret in a form field", () => {
    const { container } = render(
      <Select label="Rolle" options={options} value="a" onChange={() => {}} />,
    );
    expect(container).toHaveTextContent("▾");
  });
});
