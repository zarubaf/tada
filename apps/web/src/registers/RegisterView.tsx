import { type ReactNode, useState } from "react";
import { t } from "../i18n";
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
  loadingLabel,
  empty,
}: RegisterViewProps<T>) {
  const { state } = register;
  const [loadingMore, setLoadingMore] = useState(false);

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
