import { type ReactNode, Suspense } from "react";
import { t } from "../i18n";
import { Page } from "./Page";
import { SkeletonLines } from "./Skeleton";

/**
 * The one Suspense boundary of a lazy route: a skeleton stands in for the page while its chunk
 * loads. It sits in `Page`, so the gutters match the loaded page and nothing jumps. `label` names
 * what loads; the default fits every page.
 */
export function RouteSuspense({ children, label }: { children: ReactNode; label?: string }) {
  return (
    <Suspense
      fallback={
        <Page>
          <SkeletonLines label={label ?? t("page-loading")} />
        </Page>
      }
    >
      {children}
    </Suspense>
  );
}
