import { type ReactNode, Suspense, useLayoutEffect } from "react";
import { t } from "../i18n";
import { usePathname, useSettleFocus } from "../router/Router";
import { ChunkBoundary } from "./ChunkBoundary";
import { InlineError } from "./InlineError";
import { Page } from "./Page";
import { SkeletonLines } from "./Skeleton";

/** Tells the router that the page is in, so that focus can move to its `h1`. */
function Loaded() {
  const settle = useSettleFocus();
  // When the page mounts and each time React shows it again after a fallback: routes that share
  // this boundary keep it mounted, and an effect would not run again then.
  // biome-ignore lint/correctness/useExhaustiveDependencies: the router's function is stable.
  useLayoutEffect(settle, []);
  return null;
}

/**
 * The one Suspense boundary of a lazy route: a skeleton stands in for the page while its chunk
 * loads. It sits in `Page`, so the gutters match the loaded page and nothing jumps. If the chunk
 * fails, the area shows the failure with „Seite neu laden“, which reloads the page, instead of
 * taking the app down. `label` names what loads; the default fits every page.
 */
export function RouteSuspense({
  children,
  label,
  bare = false,
  width = "full",
}: {
  children: ReactNode;
  label?: string;
  /** For a page that sits in a `main` already, for example the event page: no second `main`. */
  bare?: boolean;
  /** The width of the page template that the page uses; see `Page`. */
  width?: "full" | "form";
}) {
  const wrap = (content: ReactNode, pending: boolean) => {
    const area = pending ? <div data-route-pending>{content}</div> : content;
    return bare ? area : <Page width={width}>{area}</Page>;
  };
  // A reset key, not a React key: the failure ends with the path, but a page that stays (for
  // example the inbox, from one proposal to the next) is not mounted again and keeps its focus.
  const pathname = usePathname();
  return (
    <ChunkBoundary
      resetKey={pathname}
      fallback={wrap(
        <InlineError
          message={t("chunk-failed")}
          retryLabel={t("chunk-failed-reload")}
          onRetry={() => window.location.reload()}
        />,
        false,
      )}
    >
      <Suspense fallback={wrap(<SkeletonLines label={label ?? t("page-loading")} />, true)}>
        {children}
        <Loaded />
      </Suspense>
    </ChunkBoundary>
  );
}
