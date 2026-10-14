// The actions of a record that a click does not run: „Ersetzen“ and „Widerrufen“ of a decision. A
// button opens the dialog that the descriptor names for it. The dialog asks what the action needs,
// runs the command and reports back; this component only decides when it is open and where focus
// goes when it closes.
import { useRef, useState } from "react";
import { t } from "../../i18n";
import { Button } from "../../ui/Button";
import { useFocusAfterCommit } from "../../ui/focus";
import type { WorkItem } from "../workItem";
import type { RecordActionSpec } from "./descriptor";
import type { FieldEnv } from "./FieldEditor";
import styles from "./RecordPanel.module.css";

export function RecordActions({
  actions,
  item,
  env,
  announce,
}: {
  actions: readonly RecordActionSpec[];
  item: WorkItem;
  env: FieldEnv;
  /** Tells the member the result of an action. */
  announce: FieldEnv["announce"];
}) {
  const [open, setOpen] = useState<string>();
  const buttons = useRef(new Map<string, HTMLButtonElement>());
  // The panel that holds the buttons: focus stays in it when the button is gone after the action.
  const panel = useRef<HTMLElement | null>(null);
  const focusAfterCommit = useFocusAfterCommit();
  const offered = actions.filter((action) => action.available(item, env));
  // The dialog that is open stays, also when the record changed meanwhile and the action is no
  // longer offered: the member keeps what they typed and reads why it failed.
  const active = actions.find((action) => action.id === open);
  if (offered.length === 0 && !active) {
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
          onPress={() => {
            panel.current = buttons.current.get(action.id)?.closest("aside") ?? null;
            setOpen(action.id);
          }}
        >
          {t(action.labelKey)}
        </Button>
      ))}
      {active && Dialog && (
        <Dialog
          item={item}
          env={env}
          onCancel={() => {
            setOpen(undefined);
            focusAfterCommit(() => buttons.current.get(active.id));
          }}
          onDone={(message) => {
            setOpen(undefined);
            announce(message, "status");
            focusAfterCommit(() => buttons.current.get(active.id) ?? panel.current);
          }}
        />
      )}
    </div>
  );
}
