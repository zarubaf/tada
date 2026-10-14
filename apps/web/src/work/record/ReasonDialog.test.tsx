import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { Problem } from "../../api/client";
import type { CommandResult } from "../workCommands";
import { ReasonDialog } from "./ReasonDialog";

const user = userEvent.setup({ delay: null });

const problem = (errors: { pointer: string; code: string }[]): Problem => ({
  type: "",
  code: "validation-failed",
  title: "",
  status: 422,
  instance: "",
  request_id: "r",
  errors,
});

const refused = (
  reason: "conflict" | "validation" | "other",
  message: string,
  withProblem?: Problem,
): CommandResult<unknown> => ({
  ok: false,
  failure: { reason, message, problem: withProblem },
});

function show(
  over: {
    onConfirm?: (reason: string) => Promise<CommandResult<unknown>>;
    danger?: boolean;
    extra?: boolean;
  } = {},
) {
  const onConfirm = over.onConfirm ?? vi.fn(async () => ({ ok: true as const, record: {} }));
  const onDone = vi.fn();
  const onCancel = vi.fn();
  render(
    <ReasonDialog
      title="COM-001 verbindlich machen"
      description={["Die Bedingung bleibt als Verlauf stehen.", "Bedingung: bei Bestellung"]}
      reasonLabel="Grund (Pflichtfeld)"
      reasonHelp="Warum ist die Bedingung erfüllt?"
      confirmLabel="Verbindlich machen"
      {...(over.danger && { danger: true })}
      {...(over.extra && { extra: <p>Eine zusätzliche Angabe</p> })}
      onConfirm={onConfirm}
      onDone={onDone}
      onCancel={onCancel}
    />,
  );
  return { onConfirm, onDone, onCancel };
}

describe("ReasonDialog", () => {
  it("is a dialog with the title, the description, the extra slot and the focused reason", () => {
    show({ extra: true });
    const dialog = screen.getByRole("dialog", { name: "COM-001 verbindlich machen" });
    expect(dialog).toBeVisible();
    expect(screen.getByText("Die Bedingung bleibt als Verlauf stehen.")).toBeVisible();
    expect(screen.getByText("Bedingung: bei Bestellung")).toBeVisible();
    expect(screen.getByText("Eine zusätzliche Angabe")).toBeVisible();
    const reason = screen.getByRole("textbox", { name: "Grund (Pflichtfeld)" });
    expect(reason).toHaveFocus();
    expect(reason).toHaveAccessibleDescription("Warum ist die Bedingung erfüllt?");
    expect(screen.getByRole("button", { name: "Verbindlich machen" })).toHaveAttribute(
      "data-variant",
      "primary",
    );
  });

  it("marks the confirm button as dangerous when it destroys something", () => {
    show({ danger: true });
    expect(screen.getByRole("button", { name: "Verbindlich machen" })).toHaveAttribute(
      "data-variant",
      "danger",
    );
  });

  it("asks for a reason and does not send without one", async () => {
    const { onConfirm } = show();
    await user.type(screen.getByRole("textbox", { name: /^Grund/ }), "   ");
    await user.click(screen.getByRole("button", { name: "Verbindlich machen" }));

    expect(
      await screen.findByText("Geben Sie einen Grund an. Er hat 1 bis 500 Zeichen."),
    ).toBeVisible();
    expect(screen.getByRole("textbox", { name: /^Grund/ })).toBeInvalid();
    expect(screen.getByRole("textbox", { name: /^Grund/ })).toHaveFocus();
    expect(onConfirm).not.toHaveBeenCalled();
  });

  it("sends the trimmed reason once, even when the button is pressed twice", async () => {
    let finish: (result: CommandResult<unknown>) => void = () => {};
    const onConfirm = vi.fn(
      () => new Promise<CommandResult<unknown>>((resolve) => (finish = resolve)),
    );
    const { onDone } = show({ onConfirm });
    await user.type(screen.getByRole("textbox", { name: /^Grund/ }), "  Auftrag unterschrieben ");
    await user.dblClick(screen.getByRole("button", { name: "Verbindlich machen" }));

    expect(onConfirm).toHaveBeenCalledTimes(1);
    expect(onConfirm).toHaveBeenCalledWith("Auftrag unterschrieben");
    expect(onDone).not.toHaveBeenCalled();
    finish({ ok: true, record: {} });
    await vi.waitFor(() => expect(onDone).toHaveBeenCalledTimes(1));
  });

  it("puts a refused reason at the field and keeps what the member typed", async () => {
    const onConfirm = vi.fn(async () =>
      refused("validation", "x", problem([{ pointer: "/reason", code: "control-character" }])),
    );
    const { onDone } = show({ onConfirm });
    await user.type(screen.getByRole("textbox", { name: /^Grund/ }), "Grund\u0001");
    await user.click(screen.getByRole("button", { name: "Verbindlich machen" }));

    expect(
      await screen.findByText("Der Grund enthält ein Zeichen, das nicht erlaubt ist."),
    ).toBeVisible();
    expect(screen.getByRole("textbox", { name: /^Grund/ })).toHaveValue("Grund\u0001");
    expect(onDone).not.toHaveBeenCalled();
  });

  it("says that the record changed after a conflict, and keeps the reason for another try", async () => {
    const onConfirm = vi.fn(async () => refused("conflict", "Version"));
    const { onDone } = show({ onConfirm });
    await user.type(screen.getByRole("textbox", { name: /^Grund/ }), "Bestellt");
    await user.click(screen.getByRole("button", { name: "Verbindlich machen" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Der Eintrag wurde inzwischen geändert und ist neu geladen. Prüfen Sie ihn und versuchen Sie es erneut.",
    );
    expect(screen.getByRole("textbox", { name: /^Grund/ })).toHaveValue("Bestellt");
    expect(onDone).not.toHaveBeenCalled();
    // The member can try again.
    onConfirm.mockResolvedValueOnce({ ok: true, record: {} } as never);
    await user.click(screen.getByRole("button", { name: "Verbindlich machen" }));
    await vi.waitFor(() => expect(onDone).toHaveBeenCalledTimes(1));
  });

  it("shows the message of another failure in an alert", async () => {
    show({ onConfirm: async () => refused("other", "Der Server antwortet nicht.") });
    await user.type(screen.getByRole("textbox", { name: /^Grund/ }), "Bestellt");
    await user.click(screen.getByRole("button", { name: "Verbindlich machen" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Der Server antwortet nicht.");
  });

  it("cancels with the button and with Escape", async () => {
    const { onCancel } = show();
    await user.click(screen.getByRole("button", { name: "Abbrechen" }));
    expect(onCancel).toHaveBeenCalledTimes(1);
    await user.keyboard("{Escape}");
    expect(onCancel).toHaveBeenCalledTimes(2);
  });

  it("does not cancel while the command runs", async () => {
    const onConfirm = vi.fn(() => new Promise<CommandResult<unknown>>(() => {}));
    const { onCancel } = show({ onConfirm });
    await user.type(screen.getByRole("textbox", { name: /^Grund/ }), "Bestellt");
    await user.click(screen.getByRole("button", { name: "Verbindlich machen" }));
    await user.keyboard("{Escape}");
    expect(onCancel).not.toHaveBeenCalled();
  });
});
