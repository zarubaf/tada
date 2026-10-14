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
      <RecordActions actions={actions} item={item} env={env(writable)} announce={announce} />
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
    expect(document.querySelector("aside")).toBeEmptyDOMElement();
  });

  it("opens the dialog of an action, and cancel returns focus to its button", async () => {
    show([revoke]);
    await user.click(screen.getByRole("button", { name: "Widerrufen" }));
    expect(screen.getByRole("dialog", { name: "Probe" })).toBeVisible();

    await user.click(screen.getByRole("button", { name: "Zurück" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(screen.getByRole("button", { name: "Widerrufen" })).toHaveFocus();
  });

  it("closes the dialog when the action is done, tells the member and keeps focus in the panel", async () => {
    const { announce } = show([revoke]);
    await user.click(screen.getByRole("button", { name: "Widerrufen" }));
    await user.click(screen.getByRole("button", { name: "Fertig" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(announce).toHaveBeenCalledWith("DEC-001 ist jetzt widerrufen.", "status");
    // The button is still there here; focus goes to it, so the member does not lose the place.
    expect(screen.getByRole("button", { name: "Widerrufen" })).toHaveFocus();
  });
});
