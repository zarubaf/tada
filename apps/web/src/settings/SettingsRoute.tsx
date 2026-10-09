import { type ReactNode, Suspense } from "react";
import { t } from "../i18n";
import { SkeletonLines } from "../ui/Skeleton";
import { SettingsLayout } from "./SettingsLayout";

/**
 * A settings route: the template, then the page. The pages load on demand, so a skeleton stands in
 * for the page while its chunk loads; the sub-navigation stays visible.
 */
export function SettingsRoute({ children }: { children: ReactNode }) {
  return (
    <SettingsLayout>
      <Suspense fallback={<SkeletonLines label={t("settings-loading")} />}>{children}</Suspense>
    </SettingsLayout>
  );
}
