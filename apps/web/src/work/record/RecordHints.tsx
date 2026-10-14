// The section „Hinweise“ of the panel: what the server or the client calculates about a record and
// that the member should know, for example a milestone with an action that is due after it. The
// section is read-only; a hint is a sentence.
import { IconAlertTriangle } from "@tabler/icons-react";
import { useId } from "react";
import { t } from "../../i18n";
import styles from "./RecordPanel.module.css";

export function RecordHints({ hints }: { hints: string[] }) {
  const headingId = useId();
  if (hints.length === 0) {
    return null;
  }
  return (
    <section className={styles.section} aria-labelledby={headingId}>
      <h3 id={headingId} className={styles.sectionHeading}>
        {t("record-hints")}
      </h3>
      <ul className={styles.hints}>
        {hints.map((hint) => (
          <li key={hint} className={styles.hint}>
            <IconAlertTriangle size={14} stroke={1.5} aria-hidden="true" />
            {hint}
          </li>
        ))}
      </ul>
    </section>
  );
}
