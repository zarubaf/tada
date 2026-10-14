import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it } from "vitest";
import { SearchSelect } from "./SearchSelect";

const user = userEvent.setup({ delay: null });

const PEOPLE = [
  { id: "a", label: "Anna Muster", leading: <span data-testid="avatar-a">AM</span> },
  { id: "b", label: "Bernd Beispiel" },
  { id: "c", label: "Clara Probe" },
];

function Harness({ initial }: { initial?: string }) {
  const [value, setValue] = useState<string | undefined>(initial);
  return (
    <>
      <SearchSelect
        label="Zuständig"
        placeholder="Niemand"
        options={PEOPLE}
        value={value}
        onChange={setValue}
        clearLabel="Niemand zuweisen"
      />
      <p data-testid="value">{value ?? "none"}</p>
    </>
  );
}

describe("SearchSelect", () => {
  it("names the trigger with the label and the value", () => {
    render(<Harness initial="b" />);

    expect(screen.getByRole("button", { name: "Zuständig: Bernd Beispiel" })).toBeInTheDocument();
  });

  it("filters the options by the typed text and picks one", async () => {
    render(<Harness />);

    await user.click(screen.getByRole("button", { name: "Zuständig: Niemand" }));
    const list = await screen.findByRole("listbox", { name: "Zuständig" });
    expect(within(list).getAllByRole("option")).toHaveLength(3);
    await user.keyboard("cla");
    expect(within(list).getAllByRole("option")).toHaveLength(1);
    await user.click(within(list).getByRole("option", { name: "Clara Probe" }));

    expect(screen.getByTestId("value")).toHaveTextContent("c");
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Zuständig: Clara Probe" })).toHaveFocus();
  });

  it("picks the highlighted option with the keyboard", async () => {
    render(<Harness />);

    await user.click(screen.getByRole("button", { name: "Zuständig: Niemand" }));
    await screen.findByRole("listbox");
    await user.keyboard("bern{ArrowDown}{Enter}");

    expect(screen.getByTestId("value")).toHaveTextContent("b");
  });

  it("closes with Escape and keeps the value", async () => {
    render(<Harness initial="a" />);

    await user.click(screen.getByRole("button", { name: /Zuständig/ }));
    await screen.findByRole("listbox");
    await user.keyboard("{Escape}");

    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    expect(screen.getByTestId("value")).toHaveTextContent("a");
    expect(screen.getByRole("button", { name: /Zuständig/ })).toHaveFocus();
  });

  it("closes when the chosen option is chosen again and returns focus to the trigger", async () => {
    render(<Harness initial="a" />);

    await user.click(screen.getByRole("button", { name: /Zuständig/ }));
    await user.click(await screen.findByRole("option", { name: /Anna Muster/ }));

    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    expect(screen.getByTestId("value")).toHaveTextContent("a");
    expect(screen.getByRole("button", { name: "Zuständig: Anna Muster" })).toHaveFocus();
  });

  it("marks the selected option and shows the leading node", async () => {
    render(<Harness initial="a" />);

    await user.click(screen.getByRole("button", { name: /Zuständig/ }));
    const selected = await screen.findByRole("option", { name: /Anna Muster/ });

    expect(selected).toHaveAttribute("aria-selected", "true");
    expect(within(selected).getByTestId("avatar-a")).toBeInTheDocument();
  });

  it("says so when nothing matches", async () => {
    render(<Harness />);

    await user.click(screen.getByRole("button", { name: /Zuständig/ }));
    await screen.findByRole("listbox");
    await user.keyboard("zzz");

    expect(await screen.findByText("Keine Treffer")).toBeInTheDocument();
  });

  it("clears the value", async () => {
    render(<Harness initial="a" />);

    await user.click(screen.getByRole("button", { name: /Zuständig/ }));
    await user.click(await screen.findByRole("button", { name: "Niemand zuweisen" }));

    expect(screen.getByTestId("value")).toHaveTextContent("none");
    expect(screen.getByRole("button", { name: "Zuständig: Niemand" })).toHaveFocus();
  });

  it("can leave out the search field for a short list", async () => {
    render(
      <SearchSelect
        label="Status"
        placeholder="offen"
        options={PEOPLE}
        value="a"
        onChange={() => {}}
        searchable={false}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Status: Anna Muster" }));

    expect(await screen.findByRole("listbox", { name: "Status" })).toBeInTheDocument();
    expect(screen.queryByRole("searchbox")).not.toBeInTheDocument();
  });

  it("shows the trigger as a quiet button on request", () => {
    render(
      <SearchSelect
        label="Zuständig"
        placeholder="Niemand"
        options={PEOPLE}
        value="a"
        onChange={() => {}}
        variant="quiet"
      />,
    );

    expect(screen.getByRole("button", { name: /Zuständig/ })).toHaveAttribute(
      "data-variant",
      "quiet",
    );
  });
});
