import {
  IconAlertTriangle,
  IconCircle,
  IconCircleCheck,
  IconCircleHalf2,
  IconCircleMinus,
  IconCircleX,
  IconHourglass,
  IconLock,
} from "@tabler/icons-react";
import type { ActionStatus, CommitmentStatus } from "../api/client";
import { t } from "../i18n";
import { StatusLabel } from "../ui/StatusLabel";

export function ActionStatusLabel({ status }: { status: ActionStatus }) {
  const parts = {
    open: { icon: IconCircle, tone: "neutral" },
    "in-progress": { icon: IconCircleHalf2, tone: "progress" },
    blocked: { icon: IconAlertTriangle, tone: "danger" },
    done: { icon: IconCircleCheck, tone: "success" },
    canceled: { icon: IconCircleMinus, tone: "muted" },
  } as const;
  return <StatusLabel {...parts[status]}>{t(`action-status-${status}`)}</StatusLabel>;
}

/** A conditional commitment is not yet reliable, so it shows in the warning tone. */
export function CommitmentStatusLabel({ status }: { status: CommitmentStatus }) {
  const parts = {
    conditional: { icon: IconHourglass, tone: "warning" },
    firm: { icon: IconLock, tone: "progress" },
    fulfilled: { icon: IconCircleCheck, tone: "success" },
    broken: { icon: IconCircleX, tone: "danger" },
    withdrawn: { icon: IconCircleMinus, tone: "muted" },
  } as const;
  return <StatusLabel {...parts[status]}>{t(`commitment-status-${status}`)}</StatusLabel>;
}
