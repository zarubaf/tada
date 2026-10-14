import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { MODELESS } from "./overlay";
import { useSlashKey } from "./useSlashKey";

function Harness({ onKey }: { onKey: () => void }) {
  useSlashKey(onKey);
  return (
    <>
      <input aria-label="Feld" />
      <button type="button">Knopf</button>
    </>
  );
}

describe("useSlashKey", () => {
  it("calls back on „/“ and keeps the browser from its quick find", async () => {
    const onKey = vi.fn();
    render(<Harness onKey={onKey} />);
    await userEvent.click(screen.getByRole("button", { name: "Knopf" }));
    const seen: KeyboardEvent[] = [];
    window.addEventListener("keydown", (event) => seen.push(event));
    await userEvent.keyboard("/");
    expect(onKey).toHaveBeenCalledTimes(1);
    expect(seen.at(-1)?.defaultPrevented).toBe(true);
  });

  it("leaves a „/“ in a text field to the field", async () => {
    const onKey = vi.fn();
    render(<Harness onKey={onKey} />);
    await userEvent.type(screen.getByRole("textbox", { name: "Feld" }), "a/b");
    expect(onKey).not.toHaveBeenCalled();
    expect(screen.getByRole("textbox", { name: "Feld" })).toHaveValue("a/b");
  });

  it("ignores a „/“ with a modifier", async () => {
    const onKey = vi.fn();
    render(<Harness onKey={onKey} />);
    await userEvent.keyboard("{Control>}/{/Control}");
    expect(onKey).not.toHaveBeenCalled();
  });

  it.each([
    ["an option of a list", "listbox", "option"],
    ["an item of a menu", "menu", "menuitem"],
  ])("leaves a „/“ on %s to that list", async (_name, container, item) => {
    const onKey = vi.fn();
    render(
      <>
        <Harness onKey={onKey} />
        <div role={container}>
          <div role={item} tabIndex={0}>
            Eintrag
          </div>
        </div>
      </>,
    );
    screen.getByRole(item).focus();
    await userEvent.keyboard("/");
    expect(onKey).not.toHaveBeenCalled();
  });

  it("leaves a „/“ to a modal dialog, such as the drawer of a phone or a picker", async () => {
    const onKey = vi.fn();
    render(
      <>
        <Harness onKey={onKey} />
        <section role="dialog" aria-label="Menü">
          <button type="button">Im Dialog</button>
        </section>
      </>,
    );
    screen.getByRole("button", { name: "Im Dialog" }).focus();
    await userEvent.keyboard("/");
    expect(onKey).not.toHaveBeenCalled();
  });

  it("leaves a „/“ to a modal dialog also when focus is behind it", async () => {
    const onKey = vi.fn();
    render(
      <>
        <Harness onKey={onKey} />
        <section role="alertdialog" aria-label="Sicher?" />
      </>,
    );
    await userEvent.click(screen.getByRole("button", { name: "Knopf" }));
    await userEvent.keyboard("/");
    expect(onKey).not.toHaveBeenCalled();
  });

  it("takes a „/“ on a control inside a popover that leaves the page usable", async () => {
    const onKey = vi.fn();
    render(
      <>
        <Harness onKey={onKey} />
        <section role="dialog" aria-label="Filter" {...{ [MODELESS]: "" }}>
          <button type="button">Verantwortlich</button>
        </section>
      </>,
    );
    screen.getByRole("button", { name: "Verantwortlich" }).focus();
    await userEvent.keyboard("/");
    expect(onKey).toHaveBeenCalledTimes(1);
  });
});
