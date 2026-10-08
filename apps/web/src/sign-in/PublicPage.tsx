import type { ReactNode, Ref } from "react";
import { t } from "../i18n";
import { Link } from "../router/Router";
import { SIGN_IN_PATH } from "../session/paths";
import { Page, PageTitle } from "../ui/Page";
import styles from "./PublicPage.module.css";

/**
 * The frame of a page without a session: a narrow column with the name of the app, one heading and
 * the content. It has no sidebar (doc/design/layout-and-responsiveness.md).
 */
export function PublicPage({
  title,
  titleRef,
  children,
}: {
  title: string;
  titleRef?: Ref<HTMLHeadingElement>;
  children: ReactNode;
}) {
  return (
    <Page width="form">
      <p className={styles.brand}>{t("app-name")}</p>
      <PageTitle ref={titleRef}>{title}</PageTitle>
      {children}
    </Page>
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
