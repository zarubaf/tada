import { IconBolt } from "@tabler/icons-react";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import * as r from "../../test/workRecords";
import { Button } from "../../ui/Button";
import type { WorkCommands } from "../workCommands";
import { toWorkItem, type WorkItem } from "../workItem";
import type { ActionDialogProps, RecordActionSpec } from "./descriptor";
import type { FieldEnv } from "./FieldEditor";
import { RecordActions } from "./RecordActions";

const user = userEvent.setup({ delay: null });

const item: WorkItem = toWorkItem("decision", r.decision("DEC-001") as never, () => "Anna Muster");

function Dialog({ onDone, onConflict, onCancel }: ActionDialogProps) {
  return (
    <div role="dialog" aria-label="Probe">
      <Button onPress={() => onDone("DEC-001 ist jetzt widerrufen.")}>Fertig</Button>
      <Button onPress={() => onDone("Nicht aktuell.", "alert")}>Veraltet</Button>
      <Button onPress={onConflict}>Konflikt</Button>
      <Button onPress={onCancel}>Zurück</Button>
    </div>
  );
}

const revoke: RecordActionSpec = {
  id: "revoke",
  labelKey: "decision-revoke",
  icon: IconBolt,
  available: (_item, env) => env.writable,
  dialog: Dialog,
};
const replace: RecordActionSpec = {
  id: "replace",
  labelKey: "decision-replace",
  available: () => false,
  dialog: Dialog,
};

const envOf = (writable: boolean) =>
  ({
    commands: {} as WorkCommands,
    refresh: vi.fn(async () => true),
    announce: vi.fn(),
    sources: { directory: undefined, milestones: undefined },
    today: "2030-05-08",
    editable: writable,
    writable,
    open: vi.fn(),
  }) satisfies FieldEnv;

function show(
  actions: readonly RecordActionSpec[],
  over: { writable?: boolean; paused?: boolean } = {},
) {
  const env = envOf(over.writable ?? true);
  const onDone = vi.fn();
  const onGone = vi.fn();
  const tree = (list: readonly RecordActionSpec[]) => (
    <aside aria-label="Panel" tabIndex={-1}>
      <h2 tabIndex={-1}>Titel</h2>
      <RecordActions
        actions={list}
        item={item}
        env={env}
        paused={over.paused ?? false}
        onDone={onDone}
        onGone={() => {
          onGone();
          screen.getByRole("heading", { name: "Titel" }).focus();
        }}
      />
    </aside>
  );
  const view = render(tree(actions));
  return {
    env,
    onDone,
    onGone,
    rerender: (list: readonly RecordActionSpec[]) => view.rerender(tree(list)),
  };
}

const menu = () => screen.getByRole("button", { name: "Aktionen" });
const choose = async (name: string) => {
  await user.click(menu());
  await user.click(await screen.findByRole("menuitem", { name }));
};

describe("RecordActions", () => {
  it("is one menu button with a popup, named „Aktionen“", () => {
    show([revoke]);
    expect(menu()).toHaveAttribute("aria-haspopup", "true");
    expect(screen.getAllByRole("button")).toHaveLength(1);
  });

  it("offers the actions that are available, in order", async () => {
    show([replace, revoke]);
    await user.click(menu());
    expect((await screen.findAllByRole("menuitem")).map((entry) => entry.textContent)).toEqual([
      "Widerrufen",
    ]);
  });

  it("opens with the keyboard and chooses with Enter", async () => {
    show([revoke]);
    menu().focus();
    await user.keyboard("{Enter}");
    expect(await screen.findByRole("menuitem", { name: "Widerrufen" })).toHaveFocus();
    await user.keyboard("{Enter}");
    expect(await screen.findByRole("dialog", { name: "Probe" })).toBeVisible();
  });

  it("is absent when no action is available, the member may not write or it is paused", () => {
    show([replace]);
    expect(screen.queryByRole("button", { name: "Aktionen" })).toBeNull();
    show([revoke], { writable: false });
    expect(screen.queryByRole("button", { name: "Aktionen" })).toBeNull();
    show([revoke], { paused: true });
    expect(screen.queryByRole("button", { name: "Aktionen" })).toBeNull();
  });

  it("opens the dialog of an action, and cancel returns focus to the menu button", async () => {
    show([revoke]);
    await choose("Widerrufen");
    expect(await screen.findByRole("dialog", { name: "Probe" })).toBeVisible();

    await user.click(screen.getByRole("button", { name: "Zurück" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await waitFor(() => expect(menu()).toHaveFocus());
  });

  it("closes the dialog when the action is done and hands the result to the caller", async () => {
    const { onDone } = show([revoke]);
    await choose("Widerrufen");
    await user.click(await screen.findByRole("button", { name: "Fertig" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(onDone).toHaveBeenCalledWith("DEC-001 ist jetzt widerrufen.", "status");
  });

  it("hands over a result that needs action as an alert", async () => {
    const { onDone } = show([revoke]);
    await choose("Widerrufen");
    await user.click(await screen.findByRole("button", { name: "Veraltet" }));
    expect(onDone).toHaveBeenCalledWith("Nicht aktuell.", "alert");
  });

  it("keeps the dialog open when the action stops being available without a conflict", async () => {
    const { rerender } = show([revoke]);
    await choose("Widerrufen");
    rerender([{ ...revoke, available: () => false }]);
    expect(screen.getByRole("dialog")).toBeVisible();
  });

  it("keeps the dialog open after a conflict while the action is still available", async () => {
    show([revoke]);
    await choose("Widerrufen");
    await user.click(await screen.findByRole("button", { name: "Konflikt" }));
    expect(screen.getByRole("dialog")).toBeVisible();
  });

  it("closes the dialog and says why when a conflict took the action away", async () => {
    const { env, onDone, onGone, rerender } = show([revoke]);
    await choose("Widerrufen");
    await user.click(await screen.findByRole("button", { name: "Konflikt" }));
    rerender([{ ...revoke, available: () => false }]);
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    // The dialog and the menu left: focus goes to the title, not to the body.
    expect(onGone).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("heading", { name: "Titel" })).toHaveFocus();
    expect(env.announce).toHaveBeenCalledWith(
      "DEC-001 hat sich inzwischen geändert, die Aktion ist nicht mehr möglich.",
      "alert",
    );
    expect(onDone).not.toHaveBeenCalled();
  });
});
