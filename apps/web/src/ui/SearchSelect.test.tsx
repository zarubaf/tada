import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { SearchSelect } from "./SearchSelect";

const user = userEvent.setup({ delay: null });

const options = [
  { id: "a", label: "Bodenbetrieb" },
  { id: "b", label: "Festwirtschaft" },
];

function show(action?: { label: string; onPress: () => void }) {
  render(
    <SearchSelect
      label="Bereich"
      placeholder="Alle Bereiche"
      options={options}
      value={undefined}
      onChange={() => {}}
      {...(action && { action })}
    />,
  );
}

describe("SearchSelect action", () => {
  it("shows no action by default", async () => {
    show();
    await user.click(screen.getByRole("button", { name: "Bereich: Alle Bereiche" }));
    expect(await screen.findByRole("listbox", { name: "Bereich" })).toBeVisible();
    expect(screen.queryByRole("button", { name: "Bereiche verwalten …" })).not.toBeInTheDocument();
  });

  it("puts the action after the list, runs it and closes the popover", async () => {
    const onPress = vi.fn();
    show({ label: "Bereiche verwalten …", onPress });
    await user.click(screen.getByRole("button", { name: "Bereich: Alle Bereiche" }));
    await user.click(await screen.findByRole("button", { name: "Bereiche verwalten …" }));

    expect(onPress).toHaveBeenCalledOnce();
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
  });
});
