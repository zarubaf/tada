import { type KeyboardEvent, type Ref, useCallback, useEffect, useRef, useState } from "react";
import {
  type Api,
  type ApplyEdit,
  type Author,
  type Changeset,
  type Field,
  type Problem,
  problemMessage,
  type ReviewResult,
} from "../api/client";
import { formatDateTime } from "../facts/formatValue";
import { t } from "../i18n";
import { Link } from "../router/Router";
import { Button } from "../ui/Button";
import { ConfirmDialog } from "../ui/ConfirmDialog";
import { useFocusAfterCommit, useRetry } from "../ui/focus";
import { InlineError } from "../ui/InlineError";
import { Skeleton } from "../ui/Skeleton";
import styles from "./ChangesetDetail.module.css";
import { conflictOf } from "./conflict";
import type { Scopes } from "./eventScopes";
import { useInbox } from "./InboxProvider";
import { fieldInfos, operationTitle } from "./OperationView";
import { editableField, ProposalCard } from "./ProposalCard";
import { deselect, select } from "./selection";
import { shortcutOf } from "./shortcuts";

type State =
  | { kind: "loading" }
  | { kind: "failed"; message: string; requestId: string | undefined }
  | { kind: "loaded"; changeset: Changeset; fields: Field[] };

export interface ChangesetDetailProps {
  api: Api;
  changesetId: string;
  /** The names of events and their time zones. */
  scopes: Scopes;
  /** A result that the member must hear: it goes to the polite live region of the page. */
  announce: (message: string | undefined) => void;
  /** A failure of an action whose control stays: it goes to the assertive live region. */
  fail: (message: string | undefined) => void;
  /** Where focus goes when the control of an action left the page. */
  ref?: Ref<HTMLElement>;
}

type Reviewed = Pick<ReviewResult, "proposals">;

function authorName(author: Author): string {
  return author.kind === "member" ? t("inbox-author-member") : t(`evidence-author-${author.kind}`);
}

/** Loads the changeset and, for an event, the field catalog behind its facts. */
async function loadChangeset(
  api: Api,
  changesetId: string,
): Promise<Exclude<State, { kind: "loading" }>> {
  try {
    const changeset = await api.GET("/api/v1/changesets/{changeset_id}", {
      params: { path: { changeset_id: changesetId } },
    });
    if (!changeset.data) {
      return {
        kind: "failed",
        message: problemMessage(changeset.error),
        requestId: changeset.error?.request_id,
      };
    }
    const eventId = changeset.data.event_id;
    if (!eventId) {
      return { kind: "loaded", changeset: changeset.data, fields: [] };
    }
    const fields = await api.GET("/api/v1/events/{event_id}/fields", {
      params: { path: { event_id: eventId } },
    });
    return fields.data
      ? { kind: "loaded", changeset: changeset.data, fields: fields.data.items }
      : {
          kind: "failed",
          message: problemMessage(fields.error),
          requestId: fields.error?.request_id,
        };
  } catch {
    return { kind: "failed", message: problemMessage(undefined), requestId: undefined };
  }
}

/** The number of proposals that a review accepted. */
function acceptedCount(result: Reviewed): number {
  return result.proposals.filter(
    (proposal) => proposal.status === "accepted" || proposal.status === "accepted-with-edit",
  ).length;
}

/**
 * The detail of the Review Inbox: the proposals of one changeset from top to bottom, the
 * selection with its summary, and the actions „Annehmen“, „Bearbeiten und annehmen“ and
 * „Ablehnen“ (ADR 0050). `A`, `E` and `R` act on the active proposal, or on the selection.
 */
export function ChangesetDetail({
  api,
  changesetId,
  scopes,
  announce,
  fail,
  ref,
}: ChangesetDetailProps) {
  const inbox = useInbox();
  const [state, setState] = useState<State>({ kind: "loading" });
  const [selected, setSelected] = useState<ReadonlySet<string>>(new Set());
  const [activeId, setActiveId] = useState<string>();
  const [editingId, setEditingId] = useState<string>();
  // The proposals that the confirmation dialog will reject.
  const [rejecting, setRejecting] = useState<string[]>();
  const [busy, setBusy] = useState(false);
  const heading = useRef<HTMLHeadingElement>(null);
  const editButtons = useRef(new Map<string, HTMLButtonElement>());
  const focusAfterCommit = useFocusAfterCommit();
  const { retried, retry } = useRetry(() => heading.current);

  /** Resolves to true when the changeset loaded. */
  const load = useCallback(async () => {
    const next = await loadChangeset(api, changesetId);
    setState(next);
    return next.kind === "loaded";
  }, [api, changesetId]);

  useEffect(() => {
    void load();
  }, [load]);

  // The loaded proposals stay on the screen while a reload runs; only the selection follows them.
  const reload = useCallback(async () => {
    const next = await loadChangeset(api, changesetId);
    if (next.kind === "loaded") {
      setState(next);
      const open = new Set(
        next.changeset.proposals.filter((p) => p.status === "open").map((p) => p.id),
      );
      setSelected((current) => new Set([...current].filter((id) => open.has(id))));
    } else {
      fail(next.message);
    }
  }, [api, changesetId, fail]);

  if (state.kind === "loading") {
    return (
      <section
        ref={ref}
        tabIndex={-1}
        aria-label={t("inbox-detail-loading")}
        className={styles.detail}
      >
        <div className={styles.skeleton} role="status" aria-label={t("inbox-detail-loading")}>
          <Skeleton />
          <Skeleton />
          <Skeleton />
        </div>
      </section>
    );
  }
  if (state.kind === "failed") {
    return (
      <section
        ref={ref}
        tabIndex={-1}
        aria-label={t("inbox-detail-loading")}
        className={styles.detail}
      >
        <InlineError
          message={state.message}
          requestId={state.requestId}
          announce={retried ? "focus" : "alert"}
          onRetry={() =>
            retry(() => {
              setState({ kind: "loading" });
              return load();
            })
          }
        />
      </section>
    );
  }

  const { changeset, fields: catalog } = state;
  const title = scopes.title(changeset.event_id);
  const timeZone = scopes.timeZone(changeset.event_id);
  const { proposals } = changeset;
  const fields = fieldInfos(changeset, catalog);
  const titleOf = (id: string) => {
    const proposal = proposals.find((candidate) => candidate.id === id);
    return proposal ? operationTitle(proposal.operation, fields) : id;
  };
  const open = proposals.filter((proposal) => proposal.status === "open");
  const active = open.find((proposal) => proposal.id === activeId) ?? open[0];

  /** The proposals that an accept of `ids` would take: the selection or one proposal, and what they need. */
  const closureOf = (ids: string[]) =>
    ids.reduce<ReadonlySet<string>>((all, id) => select(proposals, all, id).selected, new Set());
  const conflictIn = (ids: ReadonlySet<string> | string[]) =>
    [...ids].some((id) => {
      const proposal = proposals.find((candidate) => candidate.id === id);
      return proposal !== undefined && conflictOf(proposal, proposals) !== undefined;
    });

  const afterReview = async (message: string) => {
    announce(message);
    setSelected(new Set());
    setEditingId(undefined);
    setRejecting(undefined);
    await Promise.all([reload(), inbox.reload()]);
    focusAfterCommit(() => heading.current);
  };

  /** Runs a review call. The controls stay mounted while it runs, so focus stays. */
  const review = async (
    call: () => Promise<{ data?: ReviewResult; error?: Problem }>,
    message: (result: ReviewResult) => string,
  ) => {
    if (busy) {
      return;
    }
    setBusy(true);
    fail(undefined);
    announce(undefined);
    try {
      const { data, error } = await call();
      if (data) {
        await afterReview(message(data));
      } else {
        fail(problemMessage(error));
        setRejecting(undefined);
        if (error?.code === "record-version-conflict" || error?.code === "invalid-transition") {
          // The server recorded the conflict; the proposals show it now.
          await Promise.all([reload(), inbox.reload()]);
        }
      }
    } catch {
      fail(problemMessage(undefined));
      setRejecting(undefined);
    } finally {
      setBusy(false);
    }
  };

  const apply = (ids: ReadonlySet<string>, edits: ApplyEdit[] = []) => {
    if (conflictIn(ids)) {
      fail(t("inbox-summary-blocked"));
      return;
    }
    return review(
      () =>
        api.POST("/api/v1/changesets/{changeset_id}/apply", {
          params: { path: { changeset_id: changesetId } },
          body: { selected: [...ids], edits },
        }),
      (result) => t("inbox-applied", { count: acceptedCount(result) }),
    );
  };

  const reject = (ids: string[]) =>
    review(
      () =>
        api.POST("/api/v1/changesets/{changeset_id}/reject", {
          params: { path: { changeset_id: changesetId } },
          body: { proposal_ids: ids },
        }),
      (result) => t("inbox-rejected", { count: result.proposals.length }),
    );

  /** The target of the shortcut and of the buttons of the selection: the selection or one proposal. */
  const targets = (id: string | undefined): string[] =>
    selected.size > 0 ? [...selected] : id ? [id] : [];

  const choose = (id: string, on: boolean) => {
    if (on) {
      const result = select(proposals, selected, id);
      setSelected(result.selected);
      announce(
        result.added.length > 0
          ? t("inbox-dependencies-added", { count: result.added.length })
          : undefined,
      );
    } else {
      const next = deselect(proposals, selected, id);
      setSelected(next);
      const removed = selected.size - next.size - 1;
      announce(removed > 0 ? t("inbox-dependents-removed", { count: removed }) : undefined);
    }
  };

  const onKeyDown = (event: KeyboardEvent) => {
    const key = shortcutOf(event);
    if (!key || busy || rejecting || editingId) {
      return;
    }
    if (key === "a" && targets(active?.id).length > 0) {
      event.preventDefault();
      void apply(closureOf(targets(active?.id)));
    } else if (key === "e" && active && editableField(active, fields)) {
      event.preventDefault();
      setEditingId(active.id);
    } else if (key === "r" && targets(active?.id).length > 0) {
      event.preventDefault();
      setRejecting(targets(active?.id));
    }
  };

  const dependencyCount = [...selected].filter((id) =>
    proposals.some((p) => selected.has(p.id) && p.depends_on.includes(id)),
  ).length;

  return (
    // The keys work where focus is inside the detail; no shortcut is global (WCAG 2.1.4).
    <section
      ref={ref}
      tabIndex={-1}
      aria-label={title}
      className={styles.detail}
      onKeyDown={onKeyDown}
    >
      <header className={styles.header}>
        <Link to="/inbox" className={styles.back}>
          {t("inbox-back")}
        </Link>
        <h2 ref={heading} tabIndex={-1} className={styles.title}>
          {t("inbox-detail-of", { title })}
        </h2>
        <p className={styles.meta}>
          {t("inbox-proposed-by", { author: authorName(changeset.author) })},{" "}
          <time dateTime={changeset.created_at}>
            {t("inbox-proposed-at", { time: formatDateTime(changeset.created_at, timeZone) })}
          </time>
        </p>
        {open.length > 0 && (
          <p className={styles.hints}>
            <span>{t("inbox-hints")}:</span>
            {(["j", "k", "a", "e", "r"] as const).map((key) => (
              <span key={key} className={styles.hintItem}>
                <kbd>{key.toUpperCase()}</kbd> {t(`inbox-hint-${key}`)}
              </span>
            ))}
          </p>
        )}
      </header>

      <ol className={styles.proposals}>
        {proposals.map((proposal) => {
          const isOpen = proposal.status === "open";
          return (
            <li key={proposal.id}>
              <ProposalCard
                proposal={proposal}
                fields={fields}
                conflict={conflictOf(proposal, proposals)}
                needs={proposal.depends_on.map(titleOf)}
                neededBy={proposals
                  .filter(
                    (other) => selected.has(other.id) && other.depends_on.includes(proposal.id),
                  )
                  .map((other) => titleOf(other.id))}
                selected={selected.has(proposal.id)}
                onSelect={(on) => choose(proposal.id, on)}
                active={active?.id === proposal.id}
                onActivate={() => setActiveId(proposal.id)}
                primaryAccept={selected.size === 0 && active?.id === proposal.id}
                isPending={busy}
                isEditing={editingId === proposal.id}
                onAccept={() => void apply(closureOf([proposal.id]))}
                onEdit={() => isOpen && setEditingId(proposal.id)}
                onReject={() => setRejecting([proposal.id])}
                onEditSubmit={(edit) =>
                  void apply(closureOf([proposal.id]), [{ proposal_id: proposal.id, state: edit }])
                }
                onEditCancel={() => {
                  setEditingId(undefined);
                  focusAfterCommit(() => editButtons.current.get(proposal.id));
                }}
                editButtonRef={(button) => {
                  if (button) {
                    editButtons.current.set(proposal.id, button);
                  } else {
                    editButtons.current.delete(proposal.id);
                  }
                }}
              />
            </li>
          );
        })}
      </ol>

      {selected.size > 0 && (
        <section className={styles.summary} aria-labelledby="inbox-summary-title">
          <h3 id="inbox-summary-title" className={styles.summaryTitle}>
            {t("inbox-summary-title")}
          </h3>
          <p>
            {t("inbox-summary-count", { count: selected.size })}
            {dependencyCount > 0 &&
              `. ${t("inbox-summary-dependencies", { count: dependencyCount })}`}
          </p>
          <ul className={styles.summaryList}>
            {[...selected].map((id) => (
              <li key={id}>{titleOf(id)}</li>
            ))}
          </ul>
          {conflictIn(selected) && <p className={styles.blocked}>{t("inbox-summary-blocked")}</p>}
          <div className={styles.summaryActions}>
            <Button
              variant="primary"
              isPending={busy}
              isDisabled={conflictIn(selected)}
              onPress={() => void apply(selected)}
            >
              {t("inbox-summary-accept")}
            </Button>
            <Button variant="danger" isPending={busy} onPress={() => setRejecting([...selected])}>
              {t("inbox-summary-reject")}
            </Button>
          </div>
        </section>
      )}

      <ConfirmDialog
        isOpen={rejecting !== undefined}
        title={t("inbox-reject-title")}
        text={t("inbox-reject-text")}
        warning={rejecting?.map(titleOf).join(", ")}
        confirmLabel={t("inbox-reject-confirm")}
        cancelLabel={t("inbox-reject-cancel")}
        isPending={busy}
        onConfirm={() => rejecting && void reject(rejecting)}
        onCancel={() => setRejecting(undefined)}
      />
    </section>
  );
}
