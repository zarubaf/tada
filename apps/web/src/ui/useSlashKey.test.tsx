import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
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
    await userEvent.keyboard("/");
    expect(onKey).toHaveBeenCalledTimes(1);
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
});
