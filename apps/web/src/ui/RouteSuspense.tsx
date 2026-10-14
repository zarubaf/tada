import { type ReactNode, Suspense, useEffect } from "react";
import { problemMessage } from "../api/client";
import { t } from "../i18n";
import { useSettleFocus } from "../router/Router";
import { ChunkBoundary } from "./ChunkBoundary";
import { InlineError } from "./InlineError";
import { Page } from "./Page";
import { SkeletonLines } from "./Skeleton";

/** Tells the router that the page is in, so that focus can move to its `h1`. */
function Loaded() {
  const settle = useSettleFocus();
  // Once, when the page mounts.
  // biome-ignore lint/correctness/useExhaustiveDependencies: the router's function is stable.
  useEffect(settle, []);
  return null;
}

/**
 * The one Suspense boundary of a lazy route: a skeleton stands in for the page while its chunk
 * loads. It sits in `Page`, so the gutters match the loaded page and nothing jumps. If the chunk
 * fails, the area shows the failure with „Erneut versuchen“, which reloads the page, instead of
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
  return (
    <ChunkBoundary
      fallback={wrap(
        <InlineError
          message={problemMessage(undefined)}
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
