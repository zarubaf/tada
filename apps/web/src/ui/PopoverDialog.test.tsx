import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useRef, useState } from "react";
import { describe, expect, it } from "vitest";
import { isPaletteKey } from "../shell/shortcut";
import { Button } from "./Button";
import { isModalOpen } from "./overlay";
import { PopoverDialog } from "./PopoverDialog";

function Harness() {
  const [open, setOpen] = useState(false);
  const button = useRef<HTMLButtonElement>(null);
  return (
    <PopoverDialog
      title="Filter"
      narrow={false}
      isOpen={open}
      onOpenChange={setOpen}
      triggerRef={button}
      trigger={<Button ref={button}>Filter</Button>}
    >
      <input aria-label="Suche" />
    </PopoverDialog>
  );
}

const trigger = () => screen.getByRole("button", { name: "Filter" });

describe("PopoverDialog", () => {
  it("opens and closes with the same button, also by mouse", async () => {
    render(<Harness />);
    await userEvent.click(trigger());
    expect(screen.getByRole("dialog", { name: "Filter" })).toBeInTheDocument();
    expect(trigger()).toHaveAttribute("aria-expanded", "true");
    await userEvent.click(trigger());
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(trigger()).toHaveAttribute("aria-expanded", "false");
  });

  it("leaves the page usable: it is no modal dialog, so the palette key still works", async () => {
    render(<Harness />);
    await userEvent.click(trigger());
    expect(isModalOpen()).toBe(false);
    expect(isPaletteKey(new KeyboardEvent("keydown", { key: "k", ctrlKey: true }))).toBe(true);
  });

  it("closes on Escape and returns focus to the button", async () => {
    render(<Harness />);
    await userEvent.click(trigger());
    screen.getByRole("textbox", { name: "Suche" }).focus();
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    await waitFor(() => expect(trigger()).toHaveFocus());
  });
});
