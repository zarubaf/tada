import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import * as r from "../../test/workRecords";
import type { CommandResult } from "../workCommands";
import { toWorkItem } from "../workItem";
import { MakeFirmDialog } from "./commitmentDialogs";
import type { FieldEnv } from "./FieldEditor";

const user = userEvent.setup({ delay: null });

const item = toWorkItem(
  "commitment",
  r.commitment("COM-001", { condition: "bei Bestellung" }) as never,
  () => "",
);

function show(
  makeFirm: () => Promise<CommandResult<unknown>> = async () => ({ ok: true, record: {} }),
) {
  const makeFirmCommand = vi.fn(makeFirm);
  const env = {
    commands: { commitment: { makeFirm: makeFirmCommand } },
    refresh: vi.fn(async () => true),
  } as unknown as FieldEnv;
  const onDone = vi.fn();
  const onConflict = vi.fn();
  const onCancel = vi.fn();
  render(
    <MakeFirmDialog
      item={item}
      env={env}
      onDone={onDone}
      onConflict={onConflict}
      onCancel={onCancel}
    />,
  );
  return { env, makeFirmCommand, onDone, onConflict, onCancel };
}

const reason = () => screen.getByRole("textbox", { name: "Grund (Pflichtfeld)" });
const submit = () => screen.getByRole("button", { name: "Verbindlich machen" });

describe("MakeFirmDialog", () => {
  it("names the commitment, says what happens and quotes the condition", () => {
    show();
    expect(screen.getByRole("dialog", { name: "COM-001 verbindlich machen" })).toBeVisible();
    expect(screen.getByText(/Die Bedingung bleibt als Verlauf stehen/)).toBeVisible();
    expect(screen.getByText(/Bedingung: bei Bestellung/)).toBeVisible();
    expect(reason()).toHaveFocus();
    expect(reason()).toHaveAccessibleDescription(
      "Warum ist die Bedingung erfüllt? 1 bis 500 Zeichen.",
    );
    expect(submit()).toHaveAttribute("data-variant", "primary");
  });

  it("needs a reason", async () => {
    const { makeFirmCommand } = show();
    await user.type(reason(), "   ");
    await user.click(submit());
    expect(await screen.findByText(/Geben Sie einen Grund an/)).toBeVisible();
    expect(reason()).toBeInvalid();
    expect(makeFirmCommand).not.toHaveBeenCalled();
  });

  it("makes the commitment firm with the trimmed reason and the version, and tells the result", async () => {
    const { makeFirmCommand, onDone } = show();
    await user.type(reason(), "  Auftrag unterschrieben ");
    await user.click(submit());
    await waitFor(() => expect(onDone).toHaveBeenCalledWith("COM-001 ist jetzt verbindlich."));
    expect(makeFirmCommand).toHaveBeenCalledWith(
      { id: item.id, version: item.version },
      "Auftrag unterschrieben",
    );
  });

  it("puts a refused reason at the field and keeps what the member typed", async () => {
    show(async () => ({
      ok: false,
      failure: {
        reason: "validation",
        message: "x",
        problem: {
          type: "",
          code: "validation-failed",
          title: "",
          status: 422,
          instance: "",
          request_id: "r",
          errors: [{ pointer: "/reason", code: "control-character" }],
        },
      },
    }));
    await user.type(reason(), "Grund");
    await user.click(submit());
    expect(await screen.findByText(/enthält ein Zeichen, das nicht erlaubt ist/)).toBeVisible();
    expect(reason()).toHaveValue("Grund");
  });

  it("says that the record changed after a conflict, tells the panel and loads the kind again", async () => {
    const { env, onConflict, onDone } = show(async () => ({
      ok: false,
      failure: { reason: "conflict", message: "Version" },
    }));
    await user.type(reason(), "Bestellt");
    await user.click(submit());
    expect(await screen.findByRole("alert")).toHaveTextContent(/inzwischen geändert/);
    expect(onConflict).toHaveBeenCalledTimes(1);
    expect(env.refresh).toHaveBeenCalledWith("commitment");
    expect(reason()).toHaveValue("Bestellt");
    expect(onDone).not.toHaveBeenCalled();
  });

  it("cancels with the button", async () => {
    const { onCancel } = show();
    await user.click(screen.getByRole("button", { name: "Abbrechen" }));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });
});
