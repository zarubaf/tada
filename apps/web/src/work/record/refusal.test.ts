import { describe, expect, it, vi } from "vitest";
import type { Problem } from "../../api/client";
import * as r from "../../test/workRecords";
import type { CommandResult } from "../workCommands";
import { toWorkItem } from "../workItem";
import type { FieldEnv } from "./FieldEditor";
import { refusalOf } from "./refusal";

const item = toWorkItem("commitment", r.commitment("COM-001") as never, () => "");

const problem = (errors: { pointer: string; code: string }[]): Problem => ({
  type: "",
  code: "validation-failed",
  title: "",
  status: 422,
  instance: "",
  request_id: "r",
  errors,
});

const failed = (
  reason: "conflict" | "validation" | "other",
  message: string,
  withProblem?: Problem,
): CommandResult<unknown> => ({ ok: false, failure: { reason, message, problem: withProblem } });

function dialog() {
  const env = { refresh: vi.fn(async () => true) } as unknown as FieldEnv;
  return { item, env, onConflict: vi.fn() };
}

describe("refusalOf", () => {
  it("is nothing when the command worked", async () => {
    expect(await refusalOf({ ok: true, record: {} }, dialog())).toBeUndefined();
  });

  it("says that the record changed after a conflict, and loads the kind again", async () => {
    const props = dialog();
    expect(await refusalOf(failed("conflict", "Version"), props)).toEqual({
      message:
        "Der Eintrag wurde inzwischen geändert und ist neu geladen. Prüfen Sie ihn und versuchen Sie es erneut.",
    });
    expect(props.onConflict).toHaveBeenCalledTimes(1);
    expect(props.env.refresh).toHaveBeenCalledWith("commitment");
  });

  it("does not say that the record is loaded when the reload failed", async () => {
    const props = dialog();
    props.env.refresh = vi.fn(async () => false);
    expect(await refusalOf(failed("conflict", "Version"), props)).toEqual({
      message:
        "Der Eintrag wurde inzwischen geändert, liess sich aber nicht neu laden. Laden Sie die Seite neu und versuchen Sie es erneut.",
    });
    expect(props.onConflict).toHaveBeenCalledTimes(1);
  });

  it("puts a refused reason at the reason field", async () => {
    const refusal = await refusalOf(
      failed("validation", "x", problem([{ pointer: "/reason", code: "control-character" }])),
      dialog(),
    );
    expect(refusal).toEqual({ reason: "Der Grund enthält ein Zeichen, das nicht erlaubt ist." });
  });

  it("gives the message of a problem that no field knows", async () => {
    const refusal = await refusalOf(
      failed("validation", "Ungültig", problem([{ pointer: "/other", code: "x" }])),
      dialog(),
    );
    expect(refusal).toEqual({ message: "Ungültig" });
  });

  it("hands the problem to the fields of the dialog, if it has its own", async () => {
    const fields = vi.fn(() => ({}));
    const refusal = await refusalOf(failed("validation", "x", problem([])), dialog(), fields);
    expect(refusal).toEqual({});
    expect(fields).toHaveBeenCalledTimes(1);
  });

  it("gives the message of any other failure, and does not load the kind", async () => {
    const props = dialog();
    expect(await refusalOf(failed("other", "Der Server antwortet nicht."), props)).toEqual({
      message: "Der Server antwortet nicht.",
    });
    expect(props.env.refresh).not.toHaveBeenCalled();
    expect(props.onConflict).not.toHaveBeenCalled();
  });
});
