import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { type Api, problemMessage } from "../api/client";
import { t } from "../i18n";
import { Button } from "../ui/Button";
import { type Column, DataTable } from "../ui/DataTable";
import { EmptyState } from "../ui/EmptyState";
import { useFocusAfterCommit, useRetry } from "../ui/focus";
import { InlineError } from "../ui/InlineError";
import { LiveRegion } from "../ui/LiveRegion";
import { Page, PageTitle } from "../ui/Page";
import { Skeleton } from "../ui/Skeleton";
import { TextField } from "../ui/TextField";
import styles from "./PartiesPage.module.css";
import { PartyForm } from "./PartyForm";
import { type Party, type PartyKind, partyApi } from "./partyApi";

type State =
  | { kind: "loading" }
  | { kind: "failed"; message: string; requestId: string | undefined }
  | { kind: "loaded"; items: Party[]; nextCursor: string | undefined };

/**
 * „Personen“ and „Institutionen“: the register of the organization, a search by name and the form
 * that creates a record. Owners and admins can change a record. The server decides each action;
 * the page shows „Bearbeiten“ only where the record says `can_change`.
 */
export function PartiesPage({ api, kind }: { api: Api; kind: PartyKind }) {
  const party = useMemo(() => partyApi(api, kind), [api, kind]);
  const person = kind === "person";
  const [state, setState] = useState<State>({ kind: "loading" });
  const [query, setQuery] = useState("");
  const [loadingMore, setLoadingMore] = useState(false);
  const [editing, setEditing] = useState<Party>();
  const [failure, setFailure] = useState<string>();
  const [confirmation, setConfirmation] = useState<string>();
  const heading = useRef<HTMLHeadingElement>(null);
  const formHeading = useRef<HTMLHeadingElement>(null);
  // The newest search wins: an older answer that arrives late is dropped.
  const latest = useRef(0);
  const focusAfterCommit = useFocusAfterCommit();
  const { retried, retry } = useRetry(() => heading.current);

  /** The first page for the current search. Resolves to true when it loaded. */
  const load = useCallback(async () => {
    const request = ++latest.current;
    let next: State;
    try {
      const q = query.trim();
      const { data, error } = await party.list(q === "" ? {} : { q });
      next = data
        ? { kind: "loaded", items: data.items, nextCursor: data.nextCursor }
        : { kind: "failed", message: problemMessage(error), requestId: error?.request_id };
    } catch {
      next = { kind: "failed", message: problemMessage(undefined), requestId: undefined };
    }
    if (request === latest.current) {
      setState(next);
    }
    return next.kind === "loaded";
  }, [party, query]);

  useEffect(() => {
    void load();
  }, [load]);

  const loadMore = async (cursor: string, items: Party[]) => {
    setFailure(undefined);
    setConfirmation(undefined);
    setLoadingMore(true);
    try {
      const q = query.trim();
      const { data, error } = await party.list({ cursor, ...(q === "" ? {} : { q }) });
      if (data) {
        setState({ kind: "loaded", items: [...items, ...data.items], nextCursor: data.nextCursor });
        setConfirmation(t("parties-loaded-more"));
        if (data.nextCursor === undefined) {
          focusAfterCommit(() => heading.current);
        }
      } else {
        setFailure(problemMessage(error));
      }
    } catch {
      setFailure(problemMessage(undefined));
    }
    setLoadingMore(false);
  };

  const start = () => {
    setFailure(undefined);
    setConfirmation(undefined);
  };

  const failed = (message: string, code: string | undefined) => {
    setFailure(code === "record-version-conflict" ? t("parties-conflict") : message);
    if (code === "record-version-conflict") {
      setEditing(undefined);
      void load();
    }
  };

  const saved = (record: Party) => {
    setConfirmation(t("parties-saved", { name: record.name }));
    if (editing) {
      setEditing(undefined);
      setState((current) =>
        current.kind === "loaded"
          ? { ...current, items: current.items.map((i) => (i.id === record.id ? record : i)) }
          : current,
      );
      focusAfterCommit(() => heading.current);
    } else if (query.trim() === "") {
      // A new record has the highest number, so it belongs at the end of the loaded rows. With a
      // search, it may not match, so the list loads again.
      setState((current) =>
        current.kind === "loaded"
          ? { ...current, items: [...current.items.filter((i) => i.id !== record.id), record] }
          : current,
      );
    } else {
      void load();
    }
  };

  const columns: Column<Party>[] = [
    { id: "id", header: t("parties-column-id"), cell: (row) => row.local_id, mono: true },
    { id: "name", header: t("parties-column-name"), cell: (row) => row.name },
    ...(person
      ? []
      : [
          {
            id: "kind",
            header: t("parties-column-kind"),
            cell: (row: Party) => ("kind" in row ? t(`institution-kind-${row.kind}`) : ""),
          },
        ]),
    { id: "email", header: t("parties-column-email"), cell: (row) => row.email ?? "" },
    { id: "phone", header: t("parties-column-phone"), cell: (row) => row.phone ?? "" },
    ...(state.kind === "loaded" && state.items.some((row) => row.can_change)
      ? [
          {
            id: "actions",
            header: t("parties-column-actions"),
            cell: (row: Party) =>
              row.can_change && (
                <Button
                  aria-label={t("parties-edit-of", { name: row.name })}
                  onPress={() => {
                    start();
                    setEditing(row);
                    focusAfterCommit(() => formHeading.current);
                  }}
                >
                  {t("parties-edit")}
                </Button>
              ),
          },
        ]
      : []),
  ];

  const title = person ? t("persons-title") : t("institutions-title");

  return (
    <Page>
      <LiveRegion kind="alert">{failure}</LiveRegion>
      <LiveRegion kind="status">{confirmation}</LiveRegion>
      <div className={styles.toolbar}>
        <PageTitle ref={heading}>{title}</PageTitle>
        <div className={styles.search}>
          <TextField
            label={t("parties-search")}
            type="search"
            value={query}
            onChange={setQuery}
            autoComplete="off"
          />
        </div>
      </div>

      {state.kind === "loading" && (
        <div className={styles.skeleton} role="status" aria-label={t("parties-loading")}>
          <Skeleton />
          <Skeleton />
          <Skeleton />
        </div>
      )}
      {state.kind === "failed" && (
        <InlineError
          message={state.message}
          requestId={state.requestId}
          onRetry={() =>
            retry(() => {
              setState({ kind: "loading" });
              return load();
            })
          }
          announce={retried ? "focus" : "alert"}
        />
      )}
      {state.kind === "loaded" && state.items.length === 0 && (
        <EmptyState
          title={query.trim() === "" ? t("parties-empty-title") : t("parties-no-match-title")}
          text={query.trim() === "" ? t("parties-empty-text") : t("parties-no-match-text")}
        />
      )}
      {state.kind === "loaded" && state.items.length > 0 && (
        <>
          <DataTable label={title} columns={columns} rows={state.items} rowKey={(row) => row.id} />
          {state.nextCursor !== undefined && (
            <div>
              <Button
                isPending={loadingMore}
                onPress={() => {
                  if (!loadingMore && state.nextCursor !== undefined) {
                    void loadMore(state.nextCursor, state.items);
                  }
                }}
              >
                {t("parties-load-more")}
              </Button>
            </div>
          )}
        </>
      )}

      <section className={styles.section} aria-labelledby="party-form-title">
        <h2 id="party-form-title" ref={formHeading} tabIndex={-1} className={styles.heading}>
          {editing
            ? t("party-change-title", { name: editing.name })
            : person
              ? t("person-create-title")
              : t("institution-create-title")}
        </h2>
        <PartyForm
          key={editing ? `${editing.id}:${editing.version}` : "new"}
          api={party}
          kind={kind}
          {...(editing && { record: editing })}
          onStart={start}
          onSaved={saved}
          onFailed={failed}
          {...(editing && { onCancel: () => setEditing(undefined) })}
        />
      </section>
    </Page>
  );
}
