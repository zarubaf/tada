import type { ReactNode } from "react";
import { t } from "../i18n";
import { Link } from "../router/Router";
import { SIGN_IN_PATH } from "../session/paths";
import styles from "./PublicPage.module.css";

/**
 * The frame of a page without a session: a narrow column with the name of the app, one heading and
 * the content. It has no sidebar (doc/design/layout-and-responsiveness.md).
 */
export function PublicPage({ title, children }: { title: string; children: ReactNode }) {
  return (
    <main id="main" className={styles.page}>
      <p className={styles.brand}>{t("app-name")}</p>
      <h1 className={styles.title}>{title}</h1>
      {children}
    </main>
  );
}

/** Text under the heading. */
export function PublicText({ children }: { children: ReactNode }) {
  return <p className={styles.text}>{children}</p>;
}

/** The way out of a link that does not work. */
export function ToSignInLink() {
  return (
    <Link to={SIGN_IN_PATH} className={styles.link}>
      {t("to-sign-in")}
    </Link>
  );
}

export const formClass = styles.form;
