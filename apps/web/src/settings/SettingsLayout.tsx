import type { ReactNode } from "react";
import { t } from "../i18n";
import { Link } from "../router/Router";
import styles from "./SettingsLayout.module.css";

/**
 * The frame of the `/settings/*` pages: the sub-navigation and the page in `children`.
 * A settings page adds its link here.
 */
export function SettingsLayout({ children }: { children: ReactNode }) {
  return (
    <main id="main" className={styles.layout}>
      <nav className={styles.nav} aria-label={t("settings-nav")}>
        <Link to="/settings/members" className={styles.link}>
          {t("settings-nav-members")}
        </Link>
        <Link to="/settings/telegram" className={styles.link}>
          {t("settings-nav-telegram")}
        </Link>
      </nav>
      {children}
    </main>
  );
}
