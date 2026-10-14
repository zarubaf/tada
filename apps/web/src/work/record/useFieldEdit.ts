// The edit of one field in the panel: a draft that the member changes, a save through the commands
// of the page, and what a failure leaves (ADR 0082). The draft stays through a failure and a
// conflict, so the member never has to type it again.
import { useCallback, useRef, useState } from "react";
import type { RecordKind } from "../../api/client";
import { t } from "../../i18n";
import { fieldErrors } from "../fieldErrors";
import type { WorkCommands } from "../workCommands";
import type { WorkItem } from "../workItem";
import type { FieldSpec, FieldValue } from "./descriptor";

export type Phase = "idle" | "editing" | "saving" | "failed";

/** What the panel gives every field: the commands and the two ways to tell the member. */
export interface FieldServices {
  commands: WorkCommands;
  /** Loads a kind again, after a conflict. */
  refresh: (kind: RecordKind) => Promise<boolean>;
  /** Tells the member a result: politely for a save, at once for a failure. */
  announce: (text: string, urgency: "status" | "alert") => void;
}

export interface FieldEdit {
  phase: Phase;
  /** What the field shows while it is not idle. */
  draft: FieldValue;
  /** Why the last save failed, or why the draft cannot be saved. */
  error: string | undefined;
  /** Starts to edit with the saved value as the draft. */
  begin: () => void;
  change: (draft: FieldValue) => void;
  /** Saves `next`, or the draft. A value equal to the saved one only ends the edit. */
  commit: (next?: FieldValue) => void;
  /** Throws the draft away. */
  cancel: () => void;
}

/** The text of a field: surrounding blanks do not count and no text is no value. */
function normalize(spec: FieldSpec, value: FieldValue): FieldValue {
  if (value === null) {
    return null;
  }
  const text = spec.control === "text" || spec.control === "multiline" ? value.trim() : value;
  return text === "" ? null : text;
}

/** The message of the server for this field if it names one, else the message of the failure. */
function messageOf(
  spec: FieldSpec,
  failure: { message: string; problem?: Parameters<typeof fieldErrors>[0] | undefined },
): string {
  const invalid =
    failure.problem?.code === "validation-failed"
      ? fieldErrors(failure.problem, [spec.pointer])[spec.pointer]
      : undefined;
  return invalid ?? failure.message;
}

export function useFieldEdit(spec: FieldSpec, item: WorkItem, services: FieldServices): FieldEdit {
  const [phase, setPhaseState] = useState<Phase>("idle");
  // The phase as the handlers of the next event see it. A blur can follow the end of an edit, for
  // example when the input leaves the page after Escape, and it must not save the draft then.
  const current = useRef<Phase>("idle");
  const setPhase = useCallback((next: Phase) => {
    current.current = next;
    setPhaseState(next);
  }, []);
  const [draft, setDraft] = useState<FieldValue>(null);
  const [error, setError] = useState<string>();
  // A save runs once at a time; blur and Enter together must not send two requests.
  const saving = useRef(false);
  const saved = spec.read(item);

  const begin = useCallback(() => {
    setDraft(saved);
    setError(undefined);
    setPhase("editing");
  }, [saved, setPhase]);

  const cancel = useCallback(() => {
    if (!saving.current) {
      setError(undefined);
      setPhase("idle");
    }
  }, [setPhase]);

  const commit = useCallback(
    (next?: FieldValue) => {
      const open = current.current === "editing" || current.current === "failed";
      // A picker saves from idle: its choice is the draft.
      const picks = current.current === "idle" && next !== undefined;
      if (saving.current || !spec.save || !(open || picks)) {
        return;
      }
      const value = normalize(spec, next === undefined ? draft : next);
      if (value === null && spec.required) {
        setDraft(value);
        setError(t(`work-error-${spec.pointer}`));
        setPhase("failed");
        return;
      }
      if (value === saved) {
        setError(undefined);
        setPhase("idle");
        return;
      }
      saving.current = true;
      setDraft(value);
      setError(undefined);
      setPhase("saving");
      const field = t(spec.labelKey);
      void spec
        .save(services.commands, item, value)
        .then((result) => {
          if (result.ok) {
            setPhase("idle");
            services.announce(t("record-saved", { field, id: item.readableId }), "status");
            return;
          }
          const { failure } = result;
          const message =
            failure.reason === "conflict" ? t("record-conflict") : messageOf(spec, failure);
          if (failure.reason === "conflict") {
            void services.refresh(item.kind);
          }
          setError(message);
          setPhase("failed");
          services.announce(
            t("record-save-failed", { field, id: item.readableId, message }),
            "alert",
          );
        })
        .finally(() => {
          saving.current = false;
        });
    },
    [spec, item, draft, saved, services, setPhase],
  );

  return { phase, draft, error, begin, change: setDraft, commit, cancel };
}
