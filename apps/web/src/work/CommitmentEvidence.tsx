import type { RecordEvidence } from "../api/client";
import { Excerpt } from "../evidence/Excerpt";
import { formatDateTime } from "../facts/formatValue";
import { t } from "../i18n";
import { Sheet } from "../ui/Sheet";
import styles from "./Work.module.css";

/**
 * The evidence of a commitment: the passages of the accepted proposals, each with the version of
 * the commitment that it produced and the capture time of its source version.
 */
export function CommitmentEvidence({
  title,
  evidence,
  timeZone,
  onClose,
}: {
  title: string;
  evidence: RecordEvidence[];
  timeZone: string;
  onClose: () => void;
}) {
  return (
    <Sheet title={title} onClose={onClose}>
      <ul className={styles.evidenceList}>
        {evidence.map((item) => (
          <li key={`${item.proposal_id}:${item.start_offset}:${item.end_offset}`}>
            <Excerpt quote={item.quote} />
            <p className={styles.meta}>
              {t("commitment-evidence-version", { version: item.record_version })}
            </p>
            {typeof item.page === "number" && (
              <p className={styles.meta}>{t("evidence-page", { page: item.page })}</p>
            )}
            <p className={styles.meta}>
              <time dateTime={item.captured_at}>
                {t("evidence-captured", { time: formatDateTime(item.captured_at, timeZone) })}
              </time>
            </p>
          </li>
        ))}
      </ul>
    </Sheet>
  );
}
