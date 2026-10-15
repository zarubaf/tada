import type { ActionStatus, CommitmentStatus, MilestoneStatus, RecordKind } from "../api/client";
import { t } from "../i18n";
import { type IconSize, StatusIcon } from "../ui/StatusIcon";
import { StatusLabel } from "../ui/StatusLabel";
import { type StatusOf, statusPresentation } from "./recordStatus";

export interface RecordStatusProps<K extends RecordKind> {
  kind: K;
  status: StatusOf[K];
}

/** The icon and the word of a status of any record kind (doc/design/components.md, StatusLabel). */
export function RecordStatusLabel<K extends RecordKind>({ kind, status }: RecordStatusProps<K>) {
  const { glyph, tone, labelKey } = statusPresentation(kind, status);
  return (
    <StatusLabel icon={glyph} tone={tone}>
      {t(labelKey)}
    </StatusLabel>
  );
}

/** The icon of a status alone, with the word as its accessible name. */
export function RecordStatusIcon<K extends RecordKind>({
  kind,
  status,
  size,
}: RecordStatusProps<K> & { size?: IconSize }) {
  const { glyph, tone, labelKey } = statusPresentation(kind, status);
  return <StatusIcon glyph={glyph} tone={tone} size={size} label={t(labelKey)} />;
}

export const ActionStatusLabel = ({ status }: { status: ActionStatus }) => (
  <RecordStatusLabel kind="action" status={status} />
);
export const CommitmentStatusLabel = ({ status }: { status: CommitmentStatus }) => (
  <RecordStatusLabel kind="commitment" status={status} />
);
export const MilestoneStatusLabel = ({ status }: { status: MilestoneStatus }) => (
  <RecordStatusLabel kind="milestone" status={status} />
);
