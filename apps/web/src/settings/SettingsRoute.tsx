import type { ReactNode } from "react";
import { t } from "../i18n";
import { RouteSuspense } from "../ui/RouteSuspense";
import { SettingsLayout } from "./SettingsLayout";

/**
 * A settings route: the template, then the page. The pages load on demand, so a skeleton stands in
 * for the page while its chunk loads; the sub-navigation stays visible.
 */
export function SettingsRoute({ children }: { children: ReactNode }) {
  return (
    <SettingsLayout>
      <RouteSuspense label={t("settings-loading")}>{children}</RouteSuspense>
    </SettingsLayout>
  );
}
