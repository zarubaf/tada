import type { MouseEvent } from "react";
import { t } from "../i18n";
import styles from "./SkipLink.module.css";

/**
 * The first focusable element of each page: it jumps to the `main` element (WCAG 2.4.1). It moves
 * focus and keeps the address: the hash of the address names an open record (ADR 0082).
 */
export function SkipLink() {
  const skip = (event: MouseEvent<HTMLAnchorElement>) => {
    event.preventDefault();
    const main = document.getElementById("main");
    main?.setAttribute("tabindex", "-1");
    main?.focus();
  };
  return (
    // biome-ignore lint/a11y/useValidAnchor: a skip link is a page anchor; the handler only keeps the hash out of the address
    <a className={styles.link} href="#main" onClick={skip}>
      {t("skip-link")}
    </a>
  );
}
