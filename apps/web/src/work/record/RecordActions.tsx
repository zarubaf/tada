// The „Aktionen“ menu in the head of the panel, and the dialog of the action that the member chose
// (ADR 0082). The descriptor lists the actions; the menu offers those that are available for the
// record now, and is absent when there is none. The dialog asks what the action needs, runs the
// command and reports back; this component decides when it is open. A cancel returns focus to the
// menu button. After an action the caller decides where focus goes, for the menu may be gone with
// the state that the record has now.
//
// The dialog stays open while the record changes under it, so that the member keeps what they typed
// and reads why it failed. After a conflict the record is loaded again; if it no longer takes the
// action, the dialog closes with a sentence.
import { IconDots } from "@tabler/icons-react";
import { useEffect, useRef, useState } from "react";
import { t } from "../../i18n";
import { useFocusAfterCommit } from "../../ui/focus";
import { IconMenu } from "../../ui/IconMenu";
import type { WorkItem } from "../workItem";
import type { RecordActionSpec } from "./descriptor";
import type { FieldEnv } from "./FieldEditor";

export function RecordActions({
  actions,
  item,
  env,
  paused = false,
  onDone,
}: {
  actions: readonly RecordActionSpec[];
  item: WorkItem;
  env: FieldEnv;
  /** No action is offered: the record shows an old state, for example after a reload that failed. */
  paused?: boolean;
  /**
   * An action ran and its dialog is closed. The message tells what happened; its urgency is
   * `alert` if the member must act on it.
   */
  onDone: (message: string, urgency: "status" | "alert") => void;
}) {
  const [chosen, setChosen] = useState<string>();
  const [conflicted, setConflicted] = useState(false);
  const trigger = useRef<HTMLButtonElement>(null);
  const focusAfterCommit = useFocusAfterCommit();
  const offered = paused ? [] : actions.filter((action) => action.available(item, env));
  const running = actions.find((action) => action.id === chosen);
  const gone = conflicted && running !== undefined && !running.available(item, env);
  const close = () => {
    setChosen(undefined);
    setConflicted(false);
  };
  useEffect(() => {
    if (gone) {
      setChosen(undefined);
      setConflicted(false);
      env.announce(t("record-action-gone", { id: item.readableId }), "alert");
    }
  }, [gone, item.readableId, env]);
  const Dialog = running?.dialog;
  return (
    <>
      {offered.length > 0 && (
        <IconMenu
          ref={trigger}
          icon={IconDots}
          label={t("record-actions")}
          items={offered.map((action) => ({
            id: action.id,
            label: t(action.labelKey),
            ...(action.icon && { icon: action.icon }),
          }))}
          onAction={setChosen}
        />
      )}
      {running && Dialog && !gone && (
        <Dialog
          item={item}
          env={env}
          onConflict={() => setConflicted(true)}
          onCancel={() => {
            close();
            focusAfterCommit(() => trigger.current);
          }}
          onDone={(message, urgency = "status") => {
            close();
            onDone(message, urgency);
          }}
        />
      )}
    </>
  );
}
