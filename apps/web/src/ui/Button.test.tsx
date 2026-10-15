import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { Button } from "./Button";

describe("Button", () => {
  it("refuses a press while it is pending, and does not announce the state", async () => {
    const onPress = vi.fn();
    const { rerender } = render(<Button onPress={onPress}>Erstellen</Button>);
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Erstellen" }));
    expect(onPress).toHaveBeenCalledTimes(1);

    rerender(
      <Button isPending onPress={onPress}>
        Erstellen
      </Button>,
    );
    const button = screen.getByRole("button", { name: "Erstellen" });
    expect(button).toHaveAttribute("aria-disabled", "true");
    await user.click(button);
    expect(onPress).toHaveBeenCalledTimes(1);

    rerender(<Button onPress={onPress}>Erstellen</Button>);
    // The announcer of React Aria points at the button by id; the button can be gone before it ends.
    expect(document.querySelector('[data-live-announcer] [role="img"]')).toBeNull();
  });

  it("does not submit a form while it is pending", async () => {
    const onSubmit = vi.fn((event: { preventDefault: () => void }) => event.preventDefault());
    render(
      <form onSubmit={onSubmit}>
        <Button type="submit" isPending>
          Erstellen
        </Button>
      </form>,
    );
    await userEvent.setup().click(screen.getByRole("button", { name: "Erstellen" }));
    expect(onSubmit).not.toHaveBeenCalled();
  });
});
