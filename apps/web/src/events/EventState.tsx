import { IconLock } from "@tabler/icons-react";
import type { Event } from "../api/client";
import { t } from "../i18n";
import { StatusLabel } from "../ui/StatusLabel";

/** The state of an event in an icon and a word: open, or closed (ADR 0071). */
export function EventState({ status, column }: { status: Event["status"]; column?: boolean }) {
  return status === "closed" ? (
    <StatusLabel icon={IconLock} tone="muted" column={column ?? false}>
      {t("event-state-closed")}
    </StatusLabel>
  ) : (
    <StatusLabel icon="open" tone="neutral" column={column ?? false}>
      {t("event-state-open")}
    </StatusLabel>
  );
}
