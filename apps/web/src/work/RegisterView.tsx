import { type ReactNode, useState } from "react";
import { t } from "../i18n";
import { Button } from "../ui/Button";
import { type Column, DataTable } from "../ui/DataTable";
import { EmptyState } from "../ui/EmptyState";
import { InlineError } from "../ui/InlineError";
import { Skeleton } from "../ui/Skeleton";
import type { Register } from "./useRegister";
import styles from "./Work.module.css";

export interface RegisterViewProps<T extends { id: string }> {
  register: Register<T>;
  /** The name of the table. */
  label: string;
  columns: Column<T>[];
  loadingLabel: string;
  empty: { title: string; text: string };
  /** For a failure to load the next page. */
  onFailure: (message: string) => void;
  /** Moves focus to the heading of the page after a retry. */
  onRetry: (load: () => Promise<boolean>) => void;
  retried: boolean;
  children?: ReactNode;
}

/** The states of a register: loading, failed, empty and the table with „Weitere laden“. */
export function RegisterView<T extends { id: string }>({
  register,
  label,
  columns,
  loadingLabel,
  empty,
  onFailure,
  onRetry,
  retried,
}: RegisterViewProps<T>) {
  const { state } = register;
  const [loadingMore, setLoadingMore] = useState(false);

  if (state.kind === "loading") {
    return (
      <div className={styles.skeleton} role="status" aria-label={loadingLabel}>
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
        onRetry={() =>
          onRetry(() => {
            register.setLoading();
            return register.reload();
          })
        }
        announce={retried ? "focus" : "alert"}
      />
    );
  }
  if (state.items.length === 0) {
    return <EmptyState title={empty.title} text={empty.text} />;
  }
  return (
    <>
      <DataTable label={label} columns={columns} rows={state.items} rowKey={(row) => row.id} />
      {state.nextCursor !== undefined && (
        <div>
          <Button
            isPending={loadingMore}
            onPress={async () => {
              if (loadingMore) {
                return;
              }
              setLoadingMore(true);
              const failure = await register.loadMore();
              setLoadingMore(false);
              if (failure) {
                onFailure(failure);
              }
            }}
          >
            {t("work-load-more")}
          </Button>
        </div>
      )}
    </>
  );
}
