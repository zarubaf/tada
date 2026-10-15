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
  it("is nothing when the command worked", () => {
    expect(refusalOf({ ok: true, record: {} }, dialog())).toBeUndefined();
  });

  it("says that the record changed after a conflict, and loads the kind again", () => {
    const props = dialog();
    expect(refusalOf(failed("conflict", "Version"), props)).toEqual({
      message:
        "Der Eintrag wurde inzwischen geändert und ist neu geladen. Prüfen Sie ihn und versuchen Sie es erneut.",
    });
    expect(props.onConflict).toHaveBeenCalledTimes(1);
    expect(props.env.refresh).toHaveBeenCalledWith("commitment");
  });

  it("puts a refused reason at the reason field", () => {
    const refusal = refusalOf(
      failed("validation", "x", problem([{ pointer: "/reason", code: "control-character" }])),
      dialog(),
    );
    expect(refusal).toEqual({ reason: "Der Grund enthält ein Zeichen, das nicht erlaubt ist." });
  });

  it("gives the message of a problem that no field knows", () => {
    const refusal = refusalOf(
      failed("validation", "Ungültig", problem([{ pointer: "/other", code: "x" }])),
      dialog(),
    );
    expect(refusal).toEqual({ message: "Ungültig" });
  });

  it("hands the problem to the fields of the dialog, if it has its own", () => {
    const fields = vi.fn(() => ({}));
    const refusal = refusalOf(failed("validation", "x", problem([])), dialog(), fields);
    expect(refusal).toEqual({});
    expect(fields).toHaveBeenCalledTimes(1);
  });

  it("gives the message of any other failure, and does not load the kind", () => {
    const props = dialog();
    expect(refusalOf(failed("other", "Der Server antwortet nicht."), props)).toEqual({
      message: "Der Server antwortet nicht.",
    });
    expect(props.env.refresh).not.toHaveBeenCalled();
    expect(props.onConflict).not.toHaveBeenCalled();
  });
});
