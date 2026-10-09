import { type ReactNode, Suspense } from "react";
import { t } from "../i18n";
import { Skeleton } from "../ui/Skeleton";
import { SettingsLayout } from "./SettingsLayout";
import styles from "./SettingsRoute.module.css";

/**
 * A settings route: the template, then the page. The pages load on demand, so a skeleton stands in
 * for the page while its chunk loads; the sub-navigation stays visible.
 */
export function SettingsRoute({ children }: { children: ReactNode }) {
  return (
    <SettingsLayout>
      <Suspense
        fallback={
          <div className={styles.skeleton} role="status" aria-label={t("settings-loading")}>
            <Skeleton />
            <Skeleton />
            <Skeleton />
          </div>
        }
      >
        {children}
      </Suspense>
    </SettingsLayout>
  );
}
