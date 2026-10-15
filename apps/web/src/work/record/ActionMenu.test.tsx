import { IconBolt } from "@tabler/icons-react";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import * as r from "../../test/workRecords";
import { Button } from "../../ui/Button";
import type { WorkCommands } from "../workCommands";
import { toWorkItem, type WorkItem } from "../workItem";
import { ActionMenu } from "./ActionMenu";
import type { ActionDialogProps, MenuAction } from "./descriptor";
import type { FieldEnv } from "./FieldEditor";

const user = userEvent.setup({ delay: null });

const item = (): WorkItem =>
  toWorkItem("commitment", r.commitment("COM-001", { text: "Zöpfe liefern" }) as never, () => "");

function Dialog({ onDone, onConflict, onCancel }: ActionDialogProps) {
  return (
    <div role="dialog" aria-label="Probe">
      <Button onPress={() => onDone("COM-001 ist jetzt verbindlich.")}>Fertig</Button>
      <Button onPress={onConflict}>Konflikt</Button>
      <Button onPress={onCancel}>Zurück</Button>
    </div>
  );
}

const action: MenuAction = {
  id: "firm",
  labelKey: "make-firm",
  icon: IconBolt,
  available: () => true,
  dialog: Dialog,
};

const envOf = (editable: boolean) =>
  ({
    commands: {} as WorkCommands,
    refresh: vi.fn(async () => true),
    announce: vi.fn(),
    sources: { directory: undefined, milestones: undefined },
    today: "2030-05-08",
    editable,
    writable: true,
    open: vi.fn(),
  }) satisfies FieldEnv;

function setup(over: { editable?: boolean; available?: boolean } = {}) {
  const env = envOf(over.editable ?? true);
  const onDone = vi.fn();
  const menu = (available: boolean) => (
    <ActionMenu
      actions={[{ ...action, available: () => available }]}
      item={item()}
      env={env}
      onDone={onDone}
    />
  );
  const view = render(menu(over.available ?? true));
  return { env, onDone, view, menu };
}

const open = async () => {
  await user.click(screen.getByRole("button", { name: "Aktionen" }));
  await user.click(await screen.findByRole("menuitem", { name: "Verbindlich machen" }));
};

describe("ActionMenu", () => {
  it("is absent without a right to change the record, or when no action is available", () => {
    setup({ editable: false });
    expect(screen.queryByRole("button", { name: "Aktionen" })).toBeNull();
    setup({ available: false });
    expect(screen.queryByRole("button", { name: "Aktionen" })).toBeNull();
  });

  it("is a menu button with a popup, and lists the available action", async () => {
    setup();
    const trigger = screen.getByRole("button", { name: "Aktionen" });
    expect(trigger).toHaveAttribute("aria-haspopup", "true");
    await user.click(trigger);
    expect(await screen.findByRole("menuitem", { name: "Verbindlich machen" })).toBeVisible();
  });

  it("opens the dialog of the chosen action", async () => {
    setup();
    await open();
    expect(await screen.findByRole("dialog", { name: "Probe" })).toBeVisible();
  });

  it("tells the result, closes the dialog and leaves the focus to the caller", async () => {
    const { env, onDone } = setup();
    await open();
    await user.click(await screen.findByRole("button", { name: "Fertig" }));
    await waitFor(() => expect(onDone).toHaveBeenCalledTimes(1));
    expect(env.announce).toHaveBeenCalledWith("COM-001 ist jetzt verbindlich.", "status");
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("gives focus back to the menu button when the member cancels", async () => {
    setup();
    await open();
    await user.click(await screen.findByRole("button", { name: "Zurück" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await waitFor(() => expect(screen.getByRole("button", { name: "Aktionen" })).toHaveFocus());
  });

  it("keeps the dialog open when the action stops being available without a conflict", async () => {
    const { menu, view } = setup();
    await open();
    view.rerender(menu(false));
    expect(screen.getByRole("dialog")).toBeVisible();
  });

  it("closes the dialog and says why when a conflict took the action away", async () => {
    const { env, onDone, menu, view } = setup();
    await open();
    await user.click(await screen.findByRole("button", { name: "Konflikt" }));
    view.rerender(menu(false));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(env.announce).toHaveBeenCalledWith(
      "COM-001 hat sich inzwischen geändert, die Aktion ist nicht mehr möglich.",
      "alert",
    );
    expect(onDone).not.toHaveBeenCalled();
  });

  it("keeps the dialog open after a conflict while the action is still available", async () => {
    setup();
    await open();
    await user.click(await screen.findByRole("button", { name: "Konflikt" }));
    expect(screen.getByRole("dialog")).toBeVisible();
  });
});
