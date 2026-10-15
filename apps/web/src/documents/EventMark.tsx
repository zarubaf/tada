import { IconLock } from "@tabler/icons-react";
import { useEventContext } from "../events/eventContext";
import { t } from "../i18n";
import { Chip } from "../ui/Chip";
import styles from "./EventMark.module.css";

/**
 * Names the event in the toolbar row of a documents page: its key as muted text, and for a closed
 * event a chip with an icon and the word, because the page is read-only then (ADR 0071).
 */
export function EventMark() {
  const { event } = useEventContext();
  return (
    <>
      <span className={styles.key}>{event.key}</span>
      {event.status === "closed" && <Chip icon={IconLock}>{t("event-closed")}</Chip>}
    </>
  );
}
