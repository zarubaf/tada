import { t } from "../i18n";
import styles from "./SkipLink.module.css";

/** The first focusable element of each page: it jumps to the `main` element (WCAG 2.4.1). */
export function SkipLink() {
  return (
    <a className={styles.link} href="#main">
      {t("skip-link")}
    </a>
  );
}
