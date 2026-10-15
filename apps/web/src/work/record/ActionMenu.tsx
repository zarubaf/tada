import { IconDots } from "@tabler/icons-react";
import { useEffect, useState } from "react";
import { t } from "../../i18n";
import { IconMenu } from "../../ui/IconMenu";
import type { WorkItem } from "../workItem";
import type { MenuAction } from "./descriptor";
import type { FieldEnv } from "./FieldEditor";

/**
 * The „Aktionen“ menu of a record and the dialog of the action that the member chose. The menu is
 * absent when the member may not change the record or no action is available, so a record without
 * one has no empty menu. The dialog stays open while the record changes under it, and the member
 * decides to send it again or to cancel. After a conflict the record is loaded again; if it no
 * longer takes the action, the dialog closes with a sentence.
 */
export function ActionMenu({
  actions,
  item,
  env,
  onDone,
}: {
  actions: readonly MenuAction[];
  item: WorkItem;
  env: FieldEnv;
  /** An action worked and its dialog is closed. The menu may be gone with the action: focus is for the caller. */
  onDone: () => void;
}) {
  const [chosen, setChosen] = useState<string>();
  const [conflicted, setConflicted] = useState(false);
  const offered = env.editable ? actions.filter((action) => action.available(item)) : [];
  const running = actions.find((action) => action.id === chosen);
  const gone = conflicted && running !== undefined && !running.available(item);
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
          onDone={(message, urgency = "status") => {
            close();
            env.announce(message, urgency);
            onDone();
          }}
          onCancel={close}
        />
      )}
    </>
  );
}
