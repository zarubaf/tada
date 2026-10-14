import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { firstInvalidField } from "../../ui/focus";
import { PeoplePicker, type Person } from "./PeoplePicker";

const user = userEvent.setup({ delay: null });

const PEOPLE: Person[] = [
  { id: "p1", label: "Anna Muster" },
  { id: "p2", label: "Bernd Beispiel" },
  { id: "p3", label: "Cäcilia Probst" },
];

/** A picker whose search filters the people by name, as the server would. */
function Harness({
  people = PEOPLE,
  initial = [],
  max,
  onChange,
}: {
  people?: Person[];
  initial?: Person[];
  max?: number;
  onChange?: (value: Person[]) => void;
}) {
  const [value, setValue] = useState(initial);
  const [text, setText] = useState("");
  return (
    <PeoplePicker
      label="Beteiligte"
      options={people.filter((p) => p.label.toLowerCase().includes(text.toLowerCase()))}
      value={value}
      onChange={(next) => {
        setValue(next);
        onChange?.(next);
      }}
      search={text}
      onSearch={setText}
      {...(max !== undefined && { max })}
    />
  );
}

const input = () => screen.getByRole("combobox", { name: "Beteiligte" });
/** The names of the options, without the initials of the avatars. */
const names = (options: HTMLElement[]) =>
  options.map((option) => option.querySelector("[title]")?.textContent);
const list = () => screen.getByRole("listbox", { name: "Beteiligte: Treffer" });

describe("the people picker", () => {
  it("is a combobox over a multi-select listbox, with every option named and unselected", () => {
    render(<Harness />);
    expect(input()).toHaveAttribute("aria-controls", list().id);
    expect(input()).toHaveAttribute("aria-expanded", "true");
    expect(list()).toHaveAttribute("aria-multiselectable", "true");
    const options = within(list()).getAllByRole("option");
    expect(names(options)).toEqual(PEOPLE.map((p) => p.label));
    for (const option of options) {
      expect(option).toHaveAttribute("aria-selected", "false");
    }
  });

  it("selects and deselects with a click, and keeps the other choices", async () => {
    const onChange = vi.fn();
    render(<Harness onChange={onChange} />);
    await user.click(screen.getByRole("option", { name: "Anna Muster" }));
    await user.click(screen.getByRole("option", { name: "Cäcilia Probst" }));
    expect(onChange).toHaveBeenLastCalledWith([PEOPLE[0], PEOPLE[2]]);
    expect(screen.getByRole("option", { name: "Anna Muster" })).toHaveAttribute(
      "aria-selected",
      "true",
    );

    await user.click(screen.getByRole("option", { name: "Anna Muster" }));
    expect(onChange).toHaveBeenLastCalledWith([PEOPLE[2]]);
    expect(screen.getByRole("option", { name: "Anna Muster" })).toHaveAttribute(
      "aria-selected",
      "false",
    );
  });

  it("lists the chosen people with a button that removes each, and keeps focus in the search", async () => {
    render(<Harness initial={[PEOPLE[1] as Person]} />);
    const chosen = screen.getByRole("list", { name: "Beteiligte: gewählt" });
    expect(within(chosen).getByText("Bernd Beispiel")).toBeVisible();

    await user.click(within(chosen).getByRole("button", { name: "Bernd Beispiel entfernen" }));
    expect(screen.queryByRole("list", { name: "Beteiligte: gewählt" })).toBeNull();
    expect(input()).toHaveFocus();
  });

  it("searches by the typed text", async () => {
    render(<Harness />);
    await user.type(input(), "ber");
    expect(names(within(list()).getAllByRole("option"))).toEqual(["Bernd Beispiel"]);
    await user.clear(input());
    await user.type(input(), "zzz");
    expect(within(list()).queryAllByRole("option")).toHaveLength(0);
    expect(screen.getByText("Keine Treffer")).toBeVisible();
  });

  it("keeps a chosen person that the search no longer lists", async () => {
    render(<Harness />);
    await user.click(screen.getByRole("option", { name: "Anna Muster" }));
    await user.type(input(), "ber");
    expect(screen.queryByRole("option", { name: "Anna Muster" })).toBeNull();
    expect(
      within(screen.getByRole("list", { name: "Beteiligte: gewählt" })).getByText("Anna Muster"),
    ).toBeVisible();
  });

  it("moves through the options with the arrow keys and toggles with Enter", async () => {
    const onChange = vi.fn();
    render(<Harness onChange={onChange} />);
    await user.click(input());
    await user.keyboard("{ArrowDown}");
    const first = screen.getByRole("option", { name: "Anna Muster" });
    expect(input()).toHaveAttribute("aria-activedescendant", first.id);

    await user.keyboard("{ArrowDown}{Enter}");
    expect(onChange).toHaveBeenLastCalledWith([PEOPLE[1]]);
    // Focus stays in the input, so the member goes on typing or moving.
    expect(input()).toHaveFocus();

    await user.keyboard("{End}");
    expect(input()).toHaveAttribute(
      "aria-activedescendant",
      screen.getByRole("option", { name: "Cäcilia Probst" }).id,
    );
    await user.keyboard("{Home}");
    expect(input()).toHaveAttribute("aria-activedescendant", first.id);
    await user.keyboard("{ArrowUp}");
    // It stops at the first option.
    expect(input()).toHaveAttribute("aria-activedescendant", first.id);

    await user.keyboard("{Enter}");
    expect(onChange).toHaveBeenLastCalledWith([PEOPLE[1], PEOPLE[0]]);
  });

  it("clears the search with Escape, and lets Escape pass when it is empty", async () => {
    const outer = vi.fn();
    render(
      // biome-ignore lint/a11y/noStaticElementInteractions: the test watches the key that passes
      <div onKeyDown={(event) => event.key === "Escape" && outer()}>
        <Harness />
      </div>,
    );
    await user.type(input(), "ber");
    await user.keyboard("{Escape}");
    expect(input()).toHaveValue("");
    expect(outer).not.toHaveBeenCalled();
    await user.keyboard("{Escape}");
    expect(outer).toHaveBeenCalledTimes(1);
  });

  it("says how many are chosen, and announces each change", async () => {
    render(<Harness max={3} />);
    expect(screen.getByRole("status")).toBeEmptyDOMElement();
    await user.click(screen.getByRole("option", { name: "Anna Muster" }));
    expect(screen.getByRole("status")).toHaveTextContent(
      "Anna Muster hinzugefügt. 1 von 3 gewählt.",
    );
    expect(screen.getByText("1 von 3 gewählt")).toBeVisible();

    await user.click(screen.getByRole("option", { name: "Anna Muster" }));
    expect(screen.getByRole("status")).toHaveTextContent("Anna Muster entfernt. 0 von 3 gewählt.");
  });

  it("stops at the cap: the other options are disabled and the words say why", async () => {
    const onChange = vi.fn();
    render(
      <Harness max={2} initial={[PEOPLE[0] as Person, PEOPLE[1] as Person]} onChange={onChange} />,
    );
    const third = screen.getByRole("option", { name: "Cäcilia Probst" });
    expect(third).toHaveAttribute("aria-disabled", "true");
    expect(screen.getByText("Höchstens 2 möglich.")).toBeVisible();

    await user.click(third);
    await user.click(input());
    await user.keyboard("{End}{Enter}");
    expect(onChange).not.toHaveBeenCalled();

    // A chosen option stays available, to take it back.
    await user.click(screen.getByRole("option", { name: "Anna Muster" }));
    expect(onChange).toHaveBeenLastCalledWith([PEOPLE[1]]);
  });

  it("caps at 50 by default", () => {
    const many = Array.from({ length: 50 }, (_, i) => ({ id: `p${i}`, label: `Person ${i}` }));
    render(<Harness people={[...many, { id: "x", label: "Extra" }]} initial={many} />);
    expect(screen.getByText("50 von 50 gewählt")).toBeVisible();
    expect(screen.getByRole("option", { name: "Extra" })).toHaveAttribute("aria-disabled", "true");
  });

  it("shows the error and marks the search invalid", () => {
    render(
      <PeoplePicker
        label="Beteiligte"
        options={[]}
        value={[]}
        onChange={() => undefined}
        search=""
        onSearch={() => undefined}
        error="Wählen Sie mindestens eine Person."
      />,
    );
    expect(input()).toBeInvalid();
    expect(input()).toHaveAccessibleDescription(/Wählen Sie mindestens eine Person/);
  });

  it("makes the search, not a remove button, the first invalid field when people are chosen", () => {
    const { container } = render(
      <PeoplePicker
        label="Beteiligte"
        options={[]}
        value={[{ id: "p1", label: "Cäcilia Probst" }]}
        onChange={() => undefined}
        search=""
        onSearch={() => undefined}
        error="Wählen Sie höchstens 50 Personen."
      />,
    );
    expect(firstInvalidField(container)).toBe(input());
  });

  it("says that the search runs, and that it failed", () => {
    const { rerender } = render(
      <PeoplePicker
        label="Beteiligte"
        options={[]}
        value={[]}
        onChange={() => undefined}
        search=""
        onSearch={() => undefined}
        loading
      />,
    );
    expect(screen.getByText("Suche läuft")).toBeVisible();
    rerender(
      <PeoplePicker
        label="Beteiligte"
        options={[]}
        value={[]}
        onChange={() => undefined}
        search=""
        onSearch={() => undefined}
        failure="Die Suche ist fehlgeschlagen."
      />,
    );
    expect(screen.getByText("Die Suche ist fehlgeschlagen.")).toBeVisible();
  });
});
