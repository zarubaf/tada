import { type ReactNode, useEffect, useState } from "react";
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
  const linked = useLinkedRow(state.kind === "loaded" ? state.items.map(rowId) : []);

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
 * The row that the fragment of the address names, once the rows are on the page: it scrolls to the
 * row and takes focus, so that a link from another page lands on it. A fragment that names no row
 * does nothing. The mark of the row fades by itself, and not at all with reduced motion.
 */
function useLinkedRow(ids: string[]): string | undefined {
  const hash = useHash();
  const found = hash !== "" && ids.includes(hash) ? hash : undefined;
  useEffect(() => {
    if (found === undefined) {
      return;
    }
    const row = document.getElementById(found);
    row?.scrollIntoView?.({ block: "center" });
    row?.focus({ preventScroll: true });
  }, [found]);
  return found;
}
