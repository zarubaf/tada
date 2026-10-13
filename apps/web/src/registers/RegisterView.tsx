import { type ReactNode, useEffect, useRef } from "react";
import { t } from "../i18n";
import { useHash } from "../router/Router";
import { Button } from "../ui/Button";
import { type Column, DataTable } from "../ui/DataTable";
import { EmptyState } from "../ui/EmptyState";
import { InlineError } from "../ui/InlineError";
import { SkeletonLines } from "../ui/Skeleton";
import type { Register } from "./useRegister";
import type { RegisterPaging } from "./useRegisterPage";

/** Whether a fragment reads as the ID of a row (`ACT-001`); other fragments, such as `#main`, name no row. */
const isRowId = (fragment: string) => /^[A-Z]{3}-\d+$/.test(fragment);

/** The most pages the search for a linked row loads before it gives up. */
const MAX_SEARCH_PAGES = 100;

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
          <Button isPending={register.loadingMore} onPress={() => void page.loadMore(register)}>
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
 * the next pages until the row appears; when the list ends or the hook has loaded
 * `MAX_SEARCH_PAGES` pages of one list, the page says so in the polite live region. While a page
 * is on its way the button shows it, and the live region stays silent. A new list (a reload)
 * starts the search again. Each fragment is handled once: a retry or „Weitere laden“ does not move
 * focus again. The mark of the row fades by itself, and not at all with reduced motion.
 */
function useLinkedRow<T extends { id: string }>(
  register: Register<T>,
  page: RegisterPaging,
  rowId: (row: T) => string,
): string | undefined {
  const fragment = useHash();
  const hash = isRowId(fragment) ? fragment : "";
  const { state, loadingMore } = register;
  const present =
    hash !== "" && state.kind === "loaded" && state.items.some((row) => rowId(row) === hash);
  const handled = useRef<string>(undefined);
  const latestHash = useRef(hash);
  latestHash.current = hash;
  // The pages that the search for one fragment has loaded of one list.
  const searched = useRef({ hash: "", generation: 0, pages: 0 });
  const { setConfirmation, setFailure } = page;

  // biome-ignore lint/correctness/useExhaustiveDependencies: `register` changes with each render; the state is the trigger
  useEffect(() => {
    if (hash === "") {
      handled.current = undefined;
      searched.current = { hash: "", generation: 0, pages: 0 };
      return;
    }
    // A page on its way changes the state when it arrives, and that runs this effect again.
    if (state.kind !== "loaded" || handled.current === hash || loadingMore) {
      return;
    }
    if (present) {
      handled.current = hash;
      const row = document.getElementById(hash);
      row?.scrollIntoView?.({ block: "center" });
      row?.focus({ preventScroll: true });
      return;
    }
    if (searched.current.hash !== hash || searched.current.generation !== state.generation) {
      searched.current = { hash, generation: state.generation, pages: 0 };
    }
    const progress = searched.current;
    if (state.nextCursor === undefined || progress.pages >= MAX_SEARCH_PAGES) {
      handled.current = hash;
      setConfirmation(t("register-row-not-found", { id: hash }));
      return;
    }
    progress.pages += 1;
    void register.loadMore().then((outcome) => {
      // A stale page needs nothing: the new list runs this effect again. A failure for a fragment
      // that is no longer the address is dropped.
      if (outcome.kind === "failed" && latestHash.current === hash) {
        handled.current = hash;
        searched.current = { hash: "", generation: 0, pages: 0 };
        setFailure(outcome.message);
      }
    });
  }, [hash, present, state, loadingMore]);
  return present ? hash : undefined;
}
