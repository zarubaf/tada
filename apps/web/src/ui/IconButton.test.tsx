import { IconX } from "@tabler/icons-react";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { IconButton } from "./IconButton";

describe("IconButton", () => {
  it("is a button named by its label", () => {
    render(<IconButton icon={IconX} label="Schliessen" onPress={() => {}} />);
    expect(screen.getByRole("button", { name: "Schliessen" })).toBeInTheDocument();
  });

  it("shows the label as a tooltip on keyboard focus", async () => {
    render(<IconButton icon={IconX} label="Schliessen" onPress={() => {}} />);
    await userEvent.tab();
    expect(await screen.findByRole("tooltip")).toHaveTextContent("Schliessen");
  });

  it("closes the tooltip with Escape", async () => {
    render(<IconButton icon={IconX} label="Schliessen" onPress={() => {}} />);
    await userEvent.tab();
    await screen.findByRole("tooltip");
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();
  });

  it("is named by its label only, so a screen reader does not read the name twice", async () => {
    render(<IconButton icon={IconX} label="Schliessen" onPress={() => {}} />);
    await userEvent.tab();
    await screen.findByRole("tooltip");
    const description = screen.getByRole("button").getAttribute("aria-describedby");
    expect(description ?? "").toBe("");
    expect(screen.getByRole("button")).toHaveAccessibleName("Schliessen");
    expect(screen.getByRole("button")).not.toHaveAccessibleDescription();
  });

  it("hides the icon from a screen reader", () => {
    render(<IconButton icon={IconX} label="Schliessen" onPress={() => {}} />);
    expect(screen.getByRole("button").querySelector("svg")).toHaveAttribute("aria-hidden", "true");
  });

  it("presses with the mouse and the keyboard", async () => {
    const onPress = vi.fn();
    render(<IconButton icon={IconX} label="Schliessen" onPress={onPress} />);
    await userEvent.click(screen.getByRole("button"));
    await userEvent.keyboard("{Enter}");
    expect(onPress).toHaveBeenCalledTimes(2);
  });

  it("does not press when disabled", async () => {
    const onPress = vi.fn();
    render(<IconButton icon={IconX} label="Schliessen" isDisabled onPress={onPress} />);
    await userEvent.click(screen.getByRole("button"));
    expect(onPress).not.toHaveBeenCalled();
  });
});
