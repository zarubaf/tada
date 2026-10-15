import { useCallback, useEffect, useRef, useState } from "react";
import {
  type Api,
  type MyAction,
  type MyCommitment,
  type MyWork,
  problemMessage,
} from "../api/client";
import { todayLocal } from "../facts/dueDate";
import { t } from "../i18n";
import { Link } from "../router/Router";
import { Chip } from "../ui/Chip";
import { type Column, DataTable } from "../ui/DataTable";
import { EmptyState } from "../ui/EmptyState";
import { useRetry } from "../ui/focus";
import { InlineError } from "../ui/InlineError";
import { LinkButton } from "../ui/LinkButton";
import { Page, PageTitle } from "../ui/Page";
import { PageToolbar } from "../ui/PageToolbar";
import { RecordId } from "../ui/RecordId";
import { redactedOr } from "../ui/Redacted";
import { Skeleton } from "../ui/Skeleton";
import styles from "./MyWorkPage.module.css";
import { columnOf } from "./recordStatus";
import { ActionStatusLabel, CommitmentStatusLabel } from "./WorkStatus";
import tableStyles from "./WorkTable.module.css";
import { mayBeOverdue } from "./workItem";
import { recordHref } from "./workLinks";

type State =
  | { kind: "loading" }
  | { kind: "failed"; message: string; requestId: string | undefined }
  | { kind: "loaded"; work: MyWork };

interface Reference {
  event_id: string;
  event_key: string;
  local_id: string;
}

/** The reference of a record, for example `FLY28/ACT-042`: the key of the event, then the ID. */
function Reference({ row }: { row: Reference }) {
  return <RecordId>{`${row.event_key}/${row.local_id}`}</RecordId>;
}

/**
 * The title of a row is the link to the record in the panel of „Arbeit“ of its event. A redacted
 * record has no title: the link then shows „entfernt“ and takes the reference as its name.
 */
function TitleLink({ row, title }: { row: Reference & { redacted?: boolean }; title: string }) {
  return (
    <Link
      to={recordHref(row.event_id, { readableId: row.local_id })}
      className={tableStyles.link}
      title={row.redacted ? undefined : title}
      aria-label={row.redacted ? `${row.event_key}/${row.local_id}` : undefined}
    >
      {redactedOr(row.redacted, title)}
    </Link>
  );
}

/** The due date; a finished record is never overdue. */
function DueDate({
  date,
  today,
  overdue,
}: {
  date: string | null | undefined;
  today: string;
  overdue: boolean;
}) {
  return date ? <Chip variant="date" date={date} today={today} overdue={overdue} /> : null;
}

/** The heading of a section with the number of its rows. */
function SectionHeading({ id, title, count }: { id: string; title: string; count: number }) {
  return (
    <h2 id={id} className={styles.heading}>
      {title} <span className={styles.count}>{count}</span>
    </h2>
  );
}

/**
 * „Meine Arbeit“, the start page: my open actions and commitments across my events, and the
 * number of proposals that I review. A reference opens its record in the panel of the
 * „Arbeit“ page of its event.
 */
export function MyWorkPage({ api }: { api: Api }) {
  const [state, setState] = useState<State>({ kind: "loading" });
  const heading = useRef<HTMLHeadingElement>(null);
  const { retried, retry } = useRetry(() => heading.current);

  const load = useCallback(async () => {
    try {
      const { data, error } = await api.GET("/api/v1/me/work");
      if (data) {
        setState({ kind: "loaded", work: data });
        return true;
      }
      setState({ kind: "failed", message: problemMessage(error), requestId: error?.request_id });
    } catch {
      setState({ kind: "failed", message: problemMessage(undefined), requestId: undefined });
    }
    return false;
  }, [api]);

  useEffect(() => {
    void load();
  }, [load]);

  const today = todayLocal();
  const actionColumns: Column<MyAction>[] = [
    { id: "id", header: t("work-column-id"), cell: (row) => <Reference row={row} />, mono: true },
    {
      id: "title",
      header: t("action-column-title"),
      cell: (row) => <TitleLink row={row} title={row.title} />,
      grow: true,
    },
    {
      id: "status",
      header: t("work-column-status"),
      cell: (row) => <ActionStatusLabel status={row.status} />,
    },
    {
      id: "due",
      header: t("work-column-due"),
      cell: (row) => (
        <DueDate
          date={row.due_date}
          today={today}
          overdue={mayBeOverdue({ kind: "action", column: columnOf("action", row.status) })}
        />
      ),
    },
  ];
  const commitmentColumns: Column<MyCommitment>[] = [
    { id: "id", header: t("work-column-id"), cell: (row) => <Reference row={row} />, mono: true },
    {
      id: "text",
      header: t("commitment-column-text"),
      cell: (row) => <TitleLink row={row} title={row.text} />,
      grow: true,
    },
    {
      id: "status",
      header: t("work-column-status"),
      cell: (row) => <CommitmentStatusLabel status={row.status} />,
    },
    {
      id: "promisor",
      header: t("commitment-column-promisor"),
      cell: (row) => redactedOr(row.promisor.redacted, row.promisor.name),
      secondary: true,
    },
    {
      id: "due",
      header: t("work-column-due"),
      cell: (row) => (
        <DueDate
          date={row.due_date}
          today={today}
          overdue={mayBeOverdue({
            kind: "commitment",
            column: columnOf("commitment", row.status),
          })}
        />
      ),
    },
  ];

  const work = state.kind === "loaded" ? state.work : undefined;
  return (
    <Page width="flush">
      <PageToolbar>
        <PageTitle ref={heading} compact>
          {t("my-work-title")}
        </PageTitle>
        {work && work.review_count > 0 && (
          <div className={styles.end}>
            <LinkButton to="/inbox" compact>
              {t("my-work-review", { count: work.review_count })}
            </LinkButton>
          </div>
        )}
      </PageToolbar>
      <div className={styles.content}>
        {state.kind === "loading" && (
          <div className={styles.skeleton} role="status" aria-label={t("my-work-loading")}>
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
        {work && work.actions.length === 0 && work.commitments.length === 0 && (
          <EmptyState title={t("my-work-empty-title")} text={t("my-work-empty-text")} />
        )}
        {work && work.actions.length > 0 && (
          <section className={styles.section} aria-labelledby="my-actions-title">
            <SectionHeading
              id="my-actions-title"
              title={t("actions-title")}
              count={work.actions.length}
            />
            <DataTable
              label={t("actions-title")}
              columns={actionColumns}
              rows={work.actions}
              rowKey={(row) => row.id}
            />
          </section>
        )}
        {work && work.commitments.length > 0 && (
          <section className={styles.section} aria-labelledby="my-commitments-title">
            <SectionHeading
              id="my-commitments-title"
              title={t("commitments-title")}
              count={work.commitments.length}
            />
            <DataTable
              label={t("commitments-title")}
              columns={commitmentColumns}
              rows={work.commitments}
              rowKey={(row) => row.id}
            />
          </section>
        )}
      </div>
    </Page>
  );
}
