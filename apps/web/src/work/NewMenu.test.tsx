import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { NewMenu } from "./NewMenu";

const user = userEvent.setup({ delay: null });

describe("NewMenu", () => {
  it("is absent when the member may create no kind", () => {
    render(<NewMenu kinds={[]} narrow={false} onCreate={() => undefined} />);
    expect(screen.queryByRole("button")).toBeNull();
  });

  it("creates the one kind directly, as a plain button", async () => {
    const onCreate = vi.fn();
    render(<NewMenu kinds={["action"]} narrow={false} onCreate={onCreate} />);
    const button = screen.getByRole("button", { name: "Neu" });
    expect(button).not.toHaveAttribute("aria-haspopup");

    await user.click(button);

    expect(onCreate).toHaveBeenCalledWith("action");
  });

  it("is a menu button that lists the kinds in order, each with its icon", async () => {
    render(
      <NewMenu
        kinds={["action", "commitment", "milestone", "decision", "risk"]}
        narrow={false}
        onCreate={() => undefined}
      />,
    );
    const button = screen.getByRole("button", { name: "Neu" });
    // `true` is the same as `menu` for a popup.
    expect(button).toHaveAttribute("aria-haspopup", "true");

    await user.click(button);

    const menu = await screen.findByRole("menu", { name: "Neu" });
    const items = within(menu).getAllByRole("menuitem");
    expect(items.map((item) => item.textContent)).toEqual([
      "Aufgabe",
      "Zusage",
      "Meilenstein",
      "Entscheid",
      "Risiko",
    ]);
    for (const item of items) {
      expect(item.querySelector("svg")).not.toBeNull();
    }
  });

  it("reports the chosen kind and returns focus to the button", async () => {
    const onCreate = vi.fn();
    render(<NewMenu kinds={["action", "risk"]} narrow onCreate={onCreate} />);
    const button = screen.getByRole("button", { name: "Neu" });
    await user.click(button);
    await user.click(await screen.findByRole("menuitem", { name: "Risiko" }));

    expect(onCreate).toHaveBeenCalledWith("risk");
    await vi.waitFor(() => expect(button).toHaveFocus());
  });

  it("chooses with the arrow keys and Enter", async () => {
    const onCreate = vi.fn();
    render(<NewMenu kinds={["action", "commitment", "risk"]} narrow={false} onCreate={onCreate} />);
    screen.getByRole("button", { name: "Neu" }).focus();
    await user.keyboard("{ArrowDown}");
    await screen.findByRole("menu");
    await user.keyboard("{ArrowDown}{Enter}");

    expect(onCreate).toHaveBeenCalledWith("commitment");
  });

  it("closes with Escape, chooses nothing and returns focus to the button", async () => {
    const onCreate = vi.fn();
    render(<NewMenu kinds={["action", "risk"]} narrow={false} onCreate={onCreate} />);
    const button = screen.getByRole("button", { name: "Neu" });
    button.focus();
    await user.keyboard("{ArrowDown}");
    await screen.findByRole("menu");
    await user.keyboard("{Escape}");

    await vi.waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
    await vi.waitFor(() => expect(button).toHaveFocus());
    expect(onCreate).not.toHaveBeenCalled();
  });
});
