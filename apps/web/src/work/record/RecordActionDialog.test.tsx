import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { TextField } from "../../ui/TextField";
import { RecordActionDialog } from "./RecordActionDialog";
import type { Refusal } from "./refusal";

const user = userEvent.setup({ delay: null });

function open(
  over: Partial<Parameters<typeof RecordActionDialog>[0]> = {},
  onConfirm: (reason: string) => Promise<Refusal | undefined> = async () => undefined,
) {
  const onCancel = vi.fn();
  const confirm = vi.fn(onConfirm);
  render(
    <RecordActionDialog
      title="DEC-001 widerrufen"
      description="Der Entscheid gilt danach als widerrufen."
      reason={{ label: "Grund (Pflichtfeld)", help: "1 bis 500 Zeichen." }}
      confirmLabel="Widerrufen"
      onConfirm={confirm}
      onCancel={onCancel}
      {...over}
    />,
  );
  return { confirm, onCancel };
}

const dialog = () => screen.getByRole("dialog", { name: "DEC-001 widerrufen" });
const reason = () => screen.getByRole("textbox", { name: /^Grund/ });

describe("RecordActionDialog", () => {
  it("is a dialog named by its title, with the description and the reason field in focus", () => {
    open();
    expect(screen.getByText("Der Entscheid gilt danach als widerrufen.")).toBeVisible();
    expect(reason()).toHaveFocus();
    expect(screen.getByText("1 bis 500 Zeichen.")).toBeVisible();
    expect(screen.getByRole("button", { name: "Widerrufen" })).toBeVisible();
    expect(screen.getByRole("button", { name: "Abbrechen" })).toBeVisible();
  });

  it("says that a danger action cannot be undone, with a warning icon and not by color alone", () => {
    open({ danger: true });
    expect(screen.getByRole("button", { name: "Widerrufen" })).toHaveAttribute(
      "data-variant",
      "danger",
    );
    const warning = screen.getByText("Das lässt sich nicht rückgängig machen.");
    expect(warning).toBeVisible();
    expect(warning.querySelector("svg")).toHaveAttribute("aria-hidden", "true");
  });

  it("has no warning for an action that can be undone", () => {
    open();
    expect(screen.queryByText("Das lässt sich nicht rückgängig machen.")).toBeNull();
  });

  it("keeps a readable ID on one line in the title and in the description", () => {
    open({ description: "Der neue Entscheid ersetzt DEC-001." });
    const title = screen.getByRole("heading", { name: "DEC-001 widerrufen" });
    expect(within(title).getByText("DEC-001").tagName).toBe("SPAN");
    expect(screen.getByText("DEC-001", { selector: "p span" })).toBeVisible();
  });

  it("needs a reason and puts focus on the field", async () => {
    const { confirm } = open();
    await user.click(screen.getByRole("button", { name: "Widerrufen" }));
    expect(await screen.findByText(/Geben Sie einen Grund an/)).toBeVisible();
    expect(reason()).toBeInvalid();
    expect(reason()).toHaveFocus();
    expect(confirm).not.toHaveBeenCalled();
  });

  it("refuses a control character in the reason", async () => {
    const { confirm } = open();
    await user.click(reason());
    await user.paste("Grund\u0007");
    await user.click(screen.getByRole("button", { name: "Widerrufen" }));
    expect(await screen.findByText(/enthält ein Zeichen, das nicht erlaubt ist/)).toBeVisible();
    expect(confirm).not.toHaveBeenCalled();
  });

  it("turns the line breaks of a pasted reason into spaces", async () => {
    const { confirm } = open();
    await user.click(reason());
    await user.paste("Erste Zeile\nZweite Zeile");
    await user.click(screen.getByRole("button", { name: "Widerrufen" }));
    expect(confirm).toHaveBeenCalledWith("Erste Zeile Zweite Zeile");
  });

  it("confirms once with the trimmed reason, and is busy meanwhile", async () => {
    let finish: (refusal?: Refusal) => void = () => undefined;
    const { confirm, onCancel } = open(
      {},
      () => new Promise<Refusal | undefined>((resolve) => (finish = resolve)),
    );
    await user.type(reason(), "  Nicht mehr aktuell ");
    await user.click(screen.getByRole("button", { name: "Widerrufen" }));
    await user.click(screen.getByRole("button", { name: /Widerrufen/ }));
    expect(confirm).toHaveBeenCalledTimes(1);
    expect(confirm).toHaveBeenCalledWith("Nicht mehr aktuell");
    expect(screen.getByRole("button", { name: "Abbrechen" })).toBeDisabled();

    // Escape does not cancel a request that runs.
    await user.keyboard("{Escape}");
    expect(onCancel).not.toHaveBeenCalled();
    finish(undefined);
    await waitFor(() => expect(screen.getByRole("button", { name: "Abbrechen" })).toBeEnabled());
  });

  it("shows the refusal of the reason at the field and keeps what was typed", async () => {
    open({}, async () => ({ reason: "Der Grund ist zu lang." }));
    await user.type(reason(), "Text");
    await user.click(screen.getByRole("button", { name: "Widerrufen" }));
    expect(await screen.findByText("Der Grund ist zu lang.")).toBeVisible();
    expect(reason()).toHaveValue("Text");
    expect(reason()).toHaveFocus();
  });

  it("says another failure as an alert and keeps the dialog open", async () => {
    const { onCancel } = open({}, async () => ({ message: "Der Eintrag wurde geändert." }));
    await user.type(reason(), "Text");
    await user.click(screen.getByRole("button", { name: "Widerrufen" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Der Eintrag wurde geändert.");
    expect(dialog()).toBeVisible();
    expect(onCancel).not.toHaveBeenCalled();
  });

  it("cancels with the button and with Escape", async () => {
    const { onCancel } = open();
    await user.click(screen.getByRole("button", { name: "Abbrechen" }));
    expect(onCancel).toHaveBeenCalledTimes(1);
    await user.keyboard("{Escape}");
    expect(onCancel).toHaveBeenCalledTimes(2);
  });

  it("holds extra fields before the reason, and an empty refusal lets them show their errors", async () => {
    const refuse = vi.fn(async () => ({}));
    open(
      {
        title: "DEC-001 ersetzen",
        reason: undefined,
        confirmLabel: "Ersetzen",
        children: <TextField label="Wortlaut" error="Der Wortlaut fehlt." autoFocus />,
      },
      refuse,
    );
    expect(screen.queryByRole("textbox", { name: /^Grund/ })).toBeNull();
    expect(screen.getByRole("textbox", { name: "Wortlaut" })).toHaveFocus();

    await user.click(screen.getByRole("button", { name: "Ersetzen" }));
    // Without a reason field, the dialog asks for none.
    expect(refuse).toHaveBeenCalledWith("");
    await waitFor(() => expect(screen.getByRole("textbox", { name: "Wortlaut" })).toHaveFocus());
  });

  it("marks a danger action", () => {
    open({ danger: true });
    expect(screen.getByRole("button", { name: "Widerrufen" })).toHaveAttribute(
      "data-variant",
      "danger",
    );
  });
});
