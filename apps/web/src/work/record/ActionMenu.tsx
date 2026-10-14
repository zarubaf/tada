import { IconDots } from "@tabler/icons-react";
import { useState } from "react";
import { t } from "../../i18n";
import { IconMenu } from "../../ui/IconMenu";
import type { WorkItem } from "../workItem";
import type { MenuAction } from "./descriptor";
import type { FieldEnv } from "./FieldEditor";
import { ReasonDialog } from "./ReasonDialog";

/**
 * The „Aktionen“ menu of a record and the step of the action that the member chose. The menu is
 * absent when the member may not change the record or no action is available, so a record without
 * one has no empty menu. The step stays open while the record changes under it (a conflict loads it
 * again), and the member decides to send it again or to cancel.
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
  /** An action worked and its step is closed. The menu may be gone with the action: focus is for the caller. */
  onDone: () => void;
}) {
  const [chosen, setChosen] = useState<string>();
  const offered = env.editable ? actions.filter((action) => action.available(item)) : [];
  const running = actions.find((action) => action.id === chosen);
  const { step } = running ?? {};
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
      {running && step && (
        <ReasonDialog
          title={step.title(item)}
          {...(step.description && { description: step.description(item) })}
          reasonLabel={t(step.reasonKey)}
          {...(step.reasonHelpKey && { reasonHelp: t(step.reasonHelpKey) })}
          confirmLabel={t(step.confirmKey)}
          {...(step.danger && { danger: true })}
          onConfirm={async (reason) => {
            const result = await running.run(env.commands, item, reason);
            if (!result.ok && result.failure.reason === "conflict") {
              void env.refresh(item.kind);
            }
            return result;
          }}
          onDone={() => {
            setChosen(undefined);
            env.announce(t(running.doneKey, { id: item.readableId }), "status");
            onDone();
          }}
          onCancel={() => setChosen(undefined)}
        />
      )}
    </>
  );
}
