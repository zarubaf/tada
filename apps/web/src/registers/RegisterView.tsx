import { type ReactNode, useEffect, useRef, useState } from "react";
import { t } from "../i18n";
import { useHash } from "../router/Router";
import { Button } from "../ui/Button";
import { type Column, DataTable } from "../ui/DataTable";
import { EmptyState } from "../ui/EmptyState";
import { InlineError } from "../ui/InlineError";
import { SkeletonLines } from "../ui/Skeleton";
import type { Register } from "./useRegister";
import type { RegisterPaging } from "./useRegisterPage";

export interface RegisterViewProps<T extends { id: string }> {
  register: Register<T>;
  /** The state of the page: it receives the retry and the next page. */
  page: RegisterPaging;
  /** The name of the table. */
  label: string;
  columns: Column<T>[];
  /** The readable ID of a row (`ACT-001`): the `id` that a link with this fragment opens. */
  rowId: (row: T) => string;
  loadingLabel: string;
  empty: { title: string; text: string };
  children?: ReactNode;
}

/** The states of a register: loading, failed, empty and the table with „Weitere laden“. */
export function RegisterView<T extends { id: string }>({
  register,
  page,
  label,
  columns,
  rowId,
  loadingLabel,
  empty,
}: RegisterViewProps<T>) {
  const { state } = register;
  const [loadingMore, setLoadingMore] = useState(false);
  const linked = useLinkedRow(register, page, rowId);

  if (state.kind === "loading") {
    return <SkeletonLines label={loadingLabel} />;
  }
  if (state.kind === "failed") {
    return (
      <InlineError
        message={state.message}
        requestId={state.requestId}
        onRetry={() =>
          page.retry(() => {
            register.setLoading();
            return register.reload();
          })
        }
        announce={page.retried ? "focus" : "alert"}
      />
    );
  }
  if (state.items.length === 0) {
    return <EmptyState title={empty.title} text={empty.text} />;
  }
  return (
    <>
      <DataTable
        label={label}
        columns={columns}
        rows={state.items}
        rowKey={(row) => row.id}
        rowId={rowId}
        linkedId={linked}
      />
      {state.nextCursor !== undefined && (
        <div>
          <Button
            isPending={loadingMore}
            onPress={async () => {
              if (loadingMore) {
                return;
              }
              setLoadingMore(true);
              await page.loadMore(register);
              setLoadingMore(false);
            }}
          >
            {t("register-load-more")}
          </Button>
        </div>
      )}
    </>
  );
}

/**
 * The row that the fragment of the address names: it scrolls to the row and takes focus, so that a
 * link from another page lands on it. A row that is not on the loaded pages makes the hook load
 * the next pages until the row appears; when the pages run out, the page says so in the polite
 * live region. Each fragment is handled once: a retry or „Weitere laden“ does not move focus again.
 * The mark of the row fades by itself, and not at all with reduced motion.
 */
function useLinkedRow<T extends { id: string }>(
  register: Register<T>,
  page: RegisterPaging,
  rowId: (row: T) => string,
): string | undefined {
  const hash = useHash();
  const { state } = register;
  const present =
    hash !== "" && state.kind === "loaded" && state.items.some((row) => rowId(row) === hash);
  const handled = useRef<string>(undefined);
  const searching = useRef(false);
  const { setConfirmation, setFailure } = page;

  // biome-ignore lint/correctness/useExhaustiveDependencies: `register` changes with each render; the state is the trigger
  useEffect(() => {
    if (hash === "") {
      handled.current = undefined;
      return;
    }
    if (state.kind !== "loaded" || handled.current === hash) {
      return;
    }
    if (present) {
      handled.current = hash;
      const row = document.getElementById(hash);
      row?.scrollIntoView?.({ block: "center" });
      row?.focus({ preventScroll: true });
      return;
    }
    if (state.nextCursor === undefined) {
      handled.current = hash;
      setConfirmation(t("register-row-not-found", { id: hash }));
      return;
    }
    if (searching.current) {
      return;
    }
    searching.current = true;
    void register.loadMore().then((result) => {
      searching.current = false;
      if ("failure" in result) {
        handled.current = hash;
        setFailure(result.failure);
      }
    });
  }, [hash, present, state]);
  return present ? hash : undefined;
}
