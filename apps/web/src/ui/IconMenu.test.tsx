import { IconBolt } from "@tabler/icons-react";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { IconMenu } from "./IconMenu";

const user = userEvent.setup({ delay: null });

function show() {
  const onAction = vi.fn();
  render(
    <IconMenu
      icon={IconBolt}
      label="Aktionen"
      items={[
        { id: "firm", label: "Verbindlich machen", icon: IconBolt },
        { id: "other", label: "Etwas anderes" },
      ]}
      onAction={onAction}
    />,
  );
  return { onAction };
}

describe("IconMenu", () => {
  it("is a button named by its label that opens a menu of the items", async () => {
    show();
    const trigger = screen.getByRole("button", { name: "Aktionen" });
    expect(trigger).toHaveAttribute("aria-expanded", "false");
    await user.click(trigger);
    const menu = await screen.findByRole("menu", { name: "Aktionen" });
    expect(menu).toBeVisible();
    expect(screen.getAllByRole("menuitem").map((item) => item.textContent)).toEqual([
      "Verbindlich machen",
      "Etwas anderes",
    ]);
  });

  it("reports the chosen item and gives focus back to the button", async () => {
    const { onAction } = show();
    await user.click(screen.getByRole("button", { name: "Aktionen" }));
    await user.click(await screen.findByRole("menuitem", { name: "Verbindlich machen" }));
    expect(onAction).toHaveBeenCalledWith("firm");
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
    await waitFor(() => expect(screen.getByRole("button", { name: "Aktionen" })).toHaveFocus());
  });

  it("works with the keyboard", async () => {
    const { onAction } = show();
    screen.getByRole("button", { name: "Aktionen" }).focus();
    await user.keyboard("{Enter}");
    await screen.findByRole("menu");
    await user.keyboard("{ArrowDown}{Enter}");
    expect(onAction).toHaveBeenCalledWith("other");
  });

  it("closes with Escape and keeps the key from the panel around it", async () => {
    const outer = vi.fn();
    render(
      // biome-ignore lint/a11y/noStaticElementInteractions: a stand-in for the panel
      <div onKeyDown={(event) => !event.defaultPrevented && outer()}>
        <IconMenu
          icon={IconBolt}
          label="Aktionen"
          items={[{ id: "a", label: "A" }]}
          onAction={() => {}}
        />
      </div>,
    );
    await user.click(screen.getByRole("button", { name: "Aktionen" }));
    await screen.findByRole("menu");
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("menu")).toBeNull();
    expect(outer).not.toHaveBeenCalled();
  });
});
