// The action that makes a conditional commitment firm (ADR 0068): „Verbindlich machen“. The
// commitment keeps the reason, so the dialog asks for it. It is a dialog of `RecordActionDialog`.
import { t } from "../../i18n";
import type { WorkItem } from "../workItem";
import type { ActionDialogProps } from "./descriptor";
import { MAX_REASON, RecordActionDialog } from "./RecordActionDialog";
import { refusalOf } from "./refusal";

function asCommitment(item: WorkItem) {
  if (item.kind !== "commitment") {
    throw new Error(`a commitment dialog met a ${item.kind}`);
  }
  return item;
}

export function MakeFirmDialog(props: ActionDialogProps) {
  const { item, env, onDone, onCancel } = props;
  const { condition } = asCommitment(item).record;
  const description = [
    t("make-firm-text"),
    ...(condition ? [t("commitment-condition-line", { condition })] : []),
  ].join(" ");
  return (
    <RecordActionDialog
      title={t("make-firm-title", { id: item.readableId })}
      description={description}
      reason={{
        label: t("make-firm-reason"),
        help: t("make-firm-reason-help"),
        maxLength: MAX_REASON,
      }}
      confirmLabel={t("make-firm-submit")}
      onCancel={onCancel}
      onConfirm={async (reason) => {
        const result = await env.commands.commitment.makeFirm(
          { id: item.id, version: item.version },
          reason,
        );
        if (result.ok) {
          onDone(t("make-firm-done", { id: item.readableId }));
        }
        return refusalOf(result, props);
      }}
    />
  );
}
