import { IconBolt } from "@tabler/icons-react";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import * as r from "../../test/workRecords";
import type { CommandResult, WorkCommands } from "../workCommands";
import { toWorkItem, type WorkItem } from "../workItem";
import { ActionMenu } from "./ActionMenu";
import type { MenuAction } from "./descriptor";
import type { FieldEnv } from "./FieldEditor";

const user = userEvent.setup({ delay: null });

const item = (over: Record<string, unknown> = {}): WorkItem =>
  toWorkItem(
    "commitment",
    r.commitment("COM-001", { text: "Zöpfe liefern", ...over }) as never,
    () => "",
  );

function setup(
  run: MenuAction["run"] = async () => ({ ok: true, record: {} }),
  over: { editable?: boolean; available?: boolean } = {},
) {
  const action: MenuAction = {
    id: "firm",
    labelKey: "make-firm",
    icon: IconBolt,
    available: () => over.available ?? true,
    step: {
      title: (it) => `${it.readableId} verbindlich machen`,
      description: () => ["Die Bedingung bleibt als Verlauf stehen."],
      reasonKey: "make-firm-reason",
      confirmKey: "make-firm-submit",
    },
    run,
    doneKey: "make-firm-done",
  };
  const env = {
    commands: {} as WorkCommands,
    refresh: vi.fn(async () => true),
    announce: vi.fn(),
    sources: { directory: undefined, milestones: undefined },
    today: "2030-05-08",
    editable: over.editable ?? true,
    writable: true,
    open: vi.fn(),
  } satisfies FieldEnv;
  const onDone = vi.fn();
  const view = render(<ActionMenu actions={[action]} item={item()} env={env} onDone={onDone} />);
  return { env, onDone, action, view };
}

const open = async () => {
  await user.click(screen.getByRole("button", { name: "Aktionen" }));
  await user.click(await screen.findByRole("menuitem", { name: "Verbindlich machen" }));
};

describe("ActionMenu", () => {
  it("is absent without a right to change the record, or when no action is available", () => {
    setup(undefined, { editable: false });
    expect(screen.queryByRole("button", { name: "Aktionen" })).toBeNull();
    setup(undefined, { available: false });
    expect(screen.queryByRole("button", { name: "Aktionen" })).toBeNull();
  });

  it("offers the available action in a menu and opens its step", async () => {
    setup();
    await open();
    const dialog = await screen.findByRole("dialog", { name: "COM-001 verbindlich machen" });
    expect(dialog).toBeVisible();
    expect(screen.getByText("Die Bedingung bleibt als Verlauf stehen.")).toBeVisible();
    expect(screen.getByRole("textbox", { name: "Grund (Pflichtfeld)" })).toHaveFocus();
  });

  it("runs the command with the reason, tells the result and closes the step", async () => {
    const run = vi.fn(async () => ({ ok: true, record: {} }) as CommandResult<unknown>);
    const { env, onDone } = setup(run);
    await open();
    await user.type(await screen.findByRole("textbox", { name: /^Grund/ }), "Bestellt");
    await user.click(screen.getByRole("button", { name: "Verbindlich machen" }));

    await waitFor(() => expect(onDone).toHaveBeenCalledTimes(1));
    expect(run).toHaveBeenCalledWith(
      env.commands,
      expect.objectContaining({ id: "rec-com-001" }),
      "Bestellt",
    );
    expect(env.announce).toHaveBeenCalledWith("COM-001 ist jetzt verbindlich.", "status");
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("gives focus back to the menu button when the member cancels", async () => {
    setup();
    await open();
    await user.click(await screen.findByRole("button", { name: "Abbrechen" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await waitFor(() => expect(screen.getByRole("button", { name: "Aktionen" })).toHaveFocus());
  });

  it("loads the kind again after a conflict and keeps the step open", async () => {
    const { env, onDone } = setup(async () => ({
      ok: false,
      failure: { reason: "conflict", message: "x" },
    }));
    await open();
    await user.type(await screen.findByRole("textbox", { name: /^Grund/ }), "Bestellt");
    await user.click(screen.getByRole("button", { name: "Verbindlich machen" }));

    await waitFor(() => expect(env.refresh).toHaveBeenCalledWith("commitment"));
    expect(await screen.findByRole("alert")).toHaveTextContent(/inzwischen geändert/);
    expect(screen.getByRole("dialog")).toBeVisible();
    expect(onDone).not.toHaveBeenCalled();
  });

  it("keeps the step open when the action stops being available under it", async () => {
    // The refresh after a conflict can bring a record that is firm already; the step stays.
    const { env, action, onDone, view } = setup();
    await open();
    view.rerender(
      <ActionMenu
        actions={[{ ...action, available: () => false }]}
        item={item()}
        env={env}
        onDone={onDone}
      />,
    );
    expect(screen.getByRole("dialog")).toBeVisible();
    expect(screen.queryByRole("button", { name: "Aktionen" })).toBeNull();
  });
});
