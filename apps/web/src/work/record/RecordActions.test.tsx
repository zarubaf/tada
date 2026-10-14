import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import * as r from "../../test/workRecords";
import { Button } from "../../ui/Button";
import { toWorkItem } from "../workItem";
import type { ActionDialogProps, RecordActionSpec } from "./descriptor";
import type { FieldEnv } from "./FieldEditor";
import { RecordActions } from "./RecordActions";

const user = userEvent.setup({ delay: null });

const item = toWorkItem("decision", r.decision("DEC-001") as never, () => "Anna Muster");

function Dialog({ onDone, onCancel }: ActionDialogProps) {
  return (
    <div role="dialog" aria-label="Probe">
      <Button onPress={() => onDone("DEC-001 ist jetzt widerrufen.")}>Fertig</Button>
      <Button onPress={() => onDone("Nicht aktuell.", "alert")}>Veraltet</Button>
      <Button onPress={onCancel}>Zurück</Button>
    </div>
  );
}

const revoke: RecordActionSpec = {
  id: "revoke",
  labelKey: "decision-revoke",
  danger: true,
  available: (_item, env) => env.writable,
  dialog: Dialog,
};
const replace: RecordActionSpec = {
  id: "replace",
  labelKey: "decision-replace",
  available: () => false,
  dialog: Dialog,
};

const env = (writable: boolean) => ({ writable }) as FieldEnv;

function show(actions: readonly RecordActionSpec[], writable = true) {
  const announce = vi.fn();
  render(
    <aside aria-label="Panel" tabIndex={-1}>
      <h2 tabIndex={-1}>Titel</h2>
      <RecordActions
        actions={actions}
        item={item}
        env={env(writable)}
        announce={announce}
        focusTitle={() => screen.getByRole("heading")}
      />
    </aside>,
  );
  return { announce };
}

describe("RecordActions", () => {
  it("offers the actions that are available, as buttons in order", () => {
    show([replace, revoke]);
    expect(screen.getAllByRole("button").map((button) => button.textContent)).toEqual([
      "Widerrufen",
    ]);
  });

  it("shows nothing when no action is available", () => {
    show([revoke], false);
    expect(screen.queryByRole("button")).toBeNull();
    // Only the title is in the panel: the component renders nothing.
    expect(document.querySelector("aside")?.children).toHaveLength(1);
  });

  it("opens the dialog of an action, and cancel returns focus to its button", async () => {
    show([revoke]);
    await user.click(screen.getByRole("button", { name: "Widerrufen" }));
    expect(screen.getByRole("dialog", { name: "Probe" })).toBeVisible();

    await user.click(screen.getByRole("button", { name: "Zurück" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(screen.getByRole("button", { name: "Widerrufen" })).toHaveFocus();
  });

  it("closes the dialog when the action is done, tells the member and moves focus to the title", async () => {
    const { announce } = show([revoke]);
    await user.click(screen.getByRole("button", { name: "Widerrufen" }));
    await user.click(screen.getByRole("button", { name: "Fertig" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(announce).toHaveBeenCalledWith("DEC-001 ist jetzt widerrufen.", "status");
    // The button leaves when the record shows its new state; the title stays.
    expect(screen.getByRole("heading")).toHaveFocus();
  });

  it("says a result that needs action as text, and offers no action for that state", async () => {
    const { announce } = show([revoke]);
    await user.click(screen.getByRole("button", { name: "Widerrufen" }));
    await user.click(screen.getByRole("button", { name: "Veraltet" }));
    expect(announce).toHaveBeenCalledWith("Nicht aktuell.", "alert");
    expect(screen.getByText("Nicht aktuell.")).toBeVisible();
    expect(screen.queryByRole("button", { name: "Widerrufen" })).toBeNull();
    expect(screen.getByRole("heading")).toHaveFocus();
  });
});
