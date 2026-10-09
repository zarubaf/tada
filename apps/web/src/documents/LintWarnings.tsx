import type { LintWarning } from "../api/client";
import { hasMessage, t } from "../i18n";
import styles from "./LintWarnings.module.css";

/** What the lint found. The list of kinds is open: a kind that this client lacks gets a general text. */
function kindText(kind: string): string {
  const id = `draft-lint-${kind}`;
  return hasMessage(id) ? t(id) : t("draft-lint-other");
}

/**
 * The warnings of the draft lint for the review: a number, a date or an amount outside a `tada:`
 * link, or raw HTML (ADR 0051). They never block; the reviewer decides.
 */
export function LintWarnings({ warnings }: { warnings: LintWarning[] }) {
  if (warnings.length === 0) {
    return null;
  }
  return (
    <div className={styles.warnings}>
      <p className={styles.title}>{t("inbox-draft-warnings", { count: warnings.length })}</p>
      <ul className={styles.list}>
        {warnings.map((warning) => (
          <li key={`${warning.line}:${warning.kind}`}>
            {t("draft-lint-line", { line: warning.line, kind: kindText(warning.kind) })}
          </li>
        ))}
      </ul>
    </div>
  );
}
