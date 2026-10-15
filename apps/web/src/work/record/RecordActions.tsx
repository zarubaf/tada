// The actions of a record that a click does not run: „Ersetzen“ and „Widerrufen“ of a decision. A
// button opens the dialog that the descriptor names for it. The dialog asks what the action needs,
// runs the command and reports back; this component only decides when it is open and where focus
// goes when it closes. After an action focus goes to the title of the record: the buttons leave
// as soon as the record shows its new state, and a title stays.
import { useRef, useState } from "react";
import { t } from "../../i18n";
import { Button } from "../../ui/Button";
import { type FocusTarget, useFocusAfterCommit } from "../../ui/focus";
import type { WorkItem } from "../workItem";
import type { RecordActionSpec } from "./descriptor";
import type { FieldEnv } from "./FieldEditor";
import styles from "./RecordPanel.module.css";

export function RecordActions({
  actions,
  item,
  env,
  announce,
  focusTitle,
}: {
  actions: readonly RecordActionSpec[];
  item: WorkItem;
  env: FieldEnv;
  /** Tells the member the result of an action. */
  announce: FieldEnv["announce"];
  /** The title of the record, where focus goes after an action. */
  focusTitle: FocusTarget;
}) {
  const [open, setOpen] = useState<string>();
  // An action that ran, but the record shows the old state: no action is offered for that state.
  const [stale, setStale] = useState<{ version: number; message: string }>();
  const buttons = useRef(new Map<string, HTMLButtonElement>());
  const focusAfterCommit = useFocusAfterCommit();
  const notice = stale?.version === item.version ? stale.message : undefined;
  const offered = notice ? [] : actions.filter((action) => action.available(item, env));
  // The dialog that is open stays, also when the record changed meanwhile and the action is no
  // longer offered: the member keeps what they typed and reads why it failed.
  const active = actions.find((action) => action.id === open);
  if (offered.length === 0 && !active && !notice) {
    return null;
  }
  const Dialog = active?.dialog;

  return (
    <div className={styles.recordActions}>
      {offered.map((action) => (
        <Button
          key={action.id}
          size="sm"
          ref={(element) => {
            if (element) {
              buttons.current.set(action.id, element);
            } else {
              buttons.current.delete(action.id);
            }
          }}
          onPress={() => setOpen(action.id)}
        >
          {t(action.labelKey)}
        </Button>
      ))}
      {notice && <p className={styles.error}>{notice}</p>}
      {active && Dialog && (
        <Dialog
          item={item}
          env={env}
          onConflict={() => undefined}
          onCancel={() => {
            setOpen(undefined);
            focusAfterCommit(() => buttons.current.get(active.id));
          }}
          onDone={(message, urgency = "status") => {
            setOpen(undefined);
            announce(message, urgency);
            if (urgency === "alert") {
              setStale({ version: item.version, message });
            }
            focusAfterCommit(focusTitle);
          }}
        />
      )}
    </div>
  );
}
