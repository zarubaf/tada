import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { Button } from "./Button";
import { Menu } from "./Menu";
import { Toolbar } from "./Toolbar";

const user = userEvent.setup({ delay: null });

function Harness() {
  return (
    <>
      <Toolbar label="Filter und Anzeige">
        <Button>Gruppieren</Button>
        <Menu
          trigger="Filter"
          label="Filter"
          items={[{ id: "mine", label: "Meine" }]}
          onAction={() => {}}
        />
        <Button>Sortieren</Button>
      </Toolbar>
      <button type="button">Danach</button>
    </>
  );
}

describe("Toolbar", () => {
  it("is a named toolbar with one tab stop", async () => {
    render(<Harness />);

    expect(screen.getByRole("toolbar", { name: "Filter und Anzeige" })).toBeInTheDocument();
    await user.tab();
    expect(screen.getByRole("button", { name: "Gruppieren" })).toHaveFocus();
    await user.tab();
    expect(screen.getByRole("button", { name: "Danach" })).toHaveFocus();
  });

  it("moves between the controls with the arrow keys", async () => {
    render(<Harness />);

    await user.tab();
    await user.keyboard("{ArrowRight}");
    expect(screen.getByRole("button", { name: "Filter" })).toHaveFocus();
    await user.keyboard("{ArrowRight}");
    expect(screen.getByRole("button", { name: "Sortieren" })).toHaveFocus();
    await user.keyboard("{ArrowLeft}");
    expect(screen.getByRole("button", { name: "Filter" })).toHaveFocus();
  });
});
