import { IconClockExclamation } from "@tabler/icons-react";
import { useCallback, useEffect, useRef, useState } from "react";
import {
  type Api,
  type MyAction,
  type MyCommitment,
  type MyWork,
  problemMessage,
} from "../api/client";
import { formatDate } from "../facts/formatValue";
import { t } from "../i18n";
import { Link } from "../router/Router";
import { type Column, DataTable } from "../ui/DataTable";
import { EmptyState } from "../ui/EmptyState";
import { useRetry } from "../ui/focus";
import { InlineError } from "../ui/InlineError";
import { Page, PageTitle } from "../ui/Page";
import { Skeleton } from "../ui/Skeleton";
import { StatusLabel } from "../ui/StatusLabel";
import { isOverdue, todayLocal } from "./dueDate";
import styles from "./MyWorkPage.module.css";
import { ActionStatusLabel, CommitmentStatusLabel } from "./WorkStatus";

type State =
  | { kind: "loading" }
  | { kind: "failed"; message: string; requestId: string | undefined }
  | { kind: "loaded"; work: MyWork };

/** The full reference of a record, for example `FLY28/ACT-042`, linked to its register. */
function Reference({
  row,
  register,
}: {
  row: { event_id: string; event_key: string; local_id: string };
  register: "actions" | "commitments";
}) {
  return (
    <Link to={`/events/${row.event_id}/${register}`}>{`${row.event_key}/${row.local_id}`}</Link>
  );
}

function DueDate({ date, today }: { date: string | null | undefined; today: string }) {
  if (!date) {
    return null;
  }
  return (
    <>
      <time dateTime={date}>{formatDate(date)}</time>
      {isOverdue(date, today) && (
        <StatusLabel icon={IconClockExclamation} tone="danger">
          {t("my-work-overdue")}
        </StatusLabel>
      )}
    </>
  );
}

/**
 * „Meine Arbeit“, the start page: my open actions and commitments across my events, and the
 * number of proposals that I review. A reference links to the register of its event; the
 * registers have no page for a single record.
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
    {
      id: "id",
      header: t("work-column-id"),
      cell: (row) => <Reference row={row} register="actions" />,
      mono: true,
    },
    { id: "title", header: t("action-column-title"), cell: (row) => row.title },
    {
      id: "due",
      header: t("work-column-due"),
      cell: (row) => <DueDate date={row.due_date} today={today} />,
      numeric: true,
    },
    {
      id: "status",
      header: t("work-column-status"),
      cell: (row) => <ActionStatusLabel status={row.status} />,
    },
  ];
  const commitmentColumns: Column<MyCommitment>[] = [
    {
      id: "id",
      header: t("work-column-id"),
      cell: (row) => <Reference row={row} register="commitments" />,
      mono: true,
    },
    { id: "text", header: t("commitment-column-text"), cell: (row) => row.text },
    { id: "promisor", header: t("commitment-column-promisor"), cell: (row) => row.promisor.name },
    {
      id: "due",
      header: t("work-column-due"),
      cell: (row) => <DueDate date={row.due_date} today={today} />,
      numeric: true,
    },
    {
      id: "status",
      header: t("work-column-status"),
      cell: (row) => <CommitmentStatusLabel status={row.status} />,
    },
  ];

  const work = state.kind === "loaded" ? state.work : undefined;
  return (
    <Page>
      <div className={styles.page}>
        <PageTitle ref={heading}>{t("my-work-title")}</PageTitle>
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
        {work && work.review_count > 0 && (
          <p>
            <Link to="/inbox">{t("my-work-review", { count: work.review_count })}</Link>
          </p>
        )}
        {work && work.actions.length === 0 && work.commitments.length === 0 && (
          <EmptyState title={t("my-work-empty-title")} text={t("my-work-empty-text")} />
        )}
        {work && work.actions.length > 0 && (
          <section className={styles.section} aria-labelledby="my-actions-title">
            <h2 id="my-actions-title" className={styles.heading}>
              {t("actions-title")}
            </h2>
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
            <h2 id="my-commitments-title" className={styles.heading}>
              {t("commitments-title")}
            </h2>
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
