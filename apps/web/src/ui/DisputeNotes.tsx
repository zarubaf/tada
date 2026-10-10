import type { DisputeNote } from "../api/client";
import { t } from "../i18n";
import styles from "./DisputeNotes.module.css";

/** The dispute notes about a record or its source, under it (ADR 0073). Nothing if there is none. */
export function DisputeNotes({ notes }: { notes: DisputeNote[] | undefined }) {
  return notes?.map((note) => (
    <span key={`${note.created_at}:${note.text}`} className={styles.note}>
      {t("dispute-note", { text: note.text })}
    </span>
  ));
}
