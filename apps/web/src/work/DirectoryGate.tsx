import type { ReactNode } from "react";
import { t } from "../i18n";
import { useRetry } from "../ui/focus";
import { InlineError } from "../ui/InlineError";
import { Skeleton } from "../ui/Skeleton";
import type { Directory, DirectoryState } from "./directory";
import styles from "./Work.module.css";

/** Shows the page only when the directory of the event has loaded. */
export function DirectoryGate({
  state,
  reload,
  children,
}: {
  state: DirectoryState;
  reload: () => Promise<boolean>;
  children: (directory: Directory) => ReactNode;
}) {
  const { retried, retry } = useRetry(() => document.querySelector<HTMLElement>("main h1"));
  if (state.kind === "loading") {
    return (
      <div className={styles.skeleton} role="status" aria-label={t("work-loading")}>
        <Skeleton />
        <Skeleton />
        <Skeleton />
      </div>
    );
  }
  if (state.kind === "failed") {
    return (
      <InlineError
        message={state.message}
        requestId={state.requestId}
        onRetry={() => retry(reload)}
        announce={retried ? "focus" : "alert"}
      />
    );
  }
  return children(state.directory);
}
