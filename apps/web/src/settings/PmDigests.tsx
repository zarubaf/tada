import {
  IconCircleCheck,
  IconCircleMinus,
  IconCircleX,
  IconClock,
  IconHelpCircle,
} from "@tabler/icons-react";
import { useCallback, useEffect, useRef, useState } from "react";
import {
  type Api,
  type Digest,
  type DigestStatus,
  type Problem,
  problemMessage,
} from "../api/client";
import { formatDateTime } from "../facts/formatValue";
import { t } from "../i18n";
import { DataTable } from "../ui/DataTable";
import { EmptyState } from "../ui/EmptyState";
import { useRetry } from "../ui/focus";
import { InlineError } from "../ui/InlineError";
import { SkeletonLines } from "../ui/Skeleton";
import { StatusLabel } from "../ui/StatusLabel";
import styles from "./PmPage.module.css";

type State =
  | { kind: "loading" }
  | { kind: "failed"; problem: Problem | undefined }
  | { kind: "loaded"; items: Digest[] };

/** An unknown outcome needs attention: tada does not send it again (ADR 0076). */
const STATUS = {
  pending: { icon: IconClock, tone: "neutral" },
  sent: { icon: IconCircleCheck, tone: "success" },
  failed: { icon: IconCircleX, tone: "danger" },
  unknown: { icon: IconHelpCircle, tone: "warning" },
  canceled: { icon: IconCircleMinus, tone: "muted" },
} as const;

export function DigestStatusLabel({ status }: { status: DigestStatus }) {
  return (
    <StatusLabel {...STATUS[status]} column>
      {t(`pm-digest-status-${status}`)}
    </StatusLabel>
  );
}

/** The last digests of the organization with their outcome, the newest first. */
export function PmDigests({ api, timeZone }: { api: Api; timeZone: string }) {
  const [state, setState] = useState<State>({ kind: "loading" });
  const heading = useRef<HTMLHeadingElement>(null);
  const { retried, retry } = useRetry(() => heading.current);

  const load = useCallback(async () => {
    try {
      const { data, error } = await api.GET("/api/v1/organization/digests");
      setState(data ? { kind: "loaded", items: data.items } : { kind: "failed", problem: error });
      return data !== undefined;
    } catch {
      setState({ kind: "failed", problem: undefined });
      return false;
    }
  }, [api]);

  useEffect(() => {
    void load();
  }, [load]);

  return (
    <section className={styles.digests} aria-labelledby="pm-digests-title">
      <h2 id="pm-digests-title" ref={heading} tabIndex={-1} className={styles.heading}>
        {t("pm-digests-title")}
      </h2>
      {state.kind === "loading" && <SkeletonLines label={t("pm-digests-loading")} />}
      {state.kind === "failed" && (
        <InlineError
          message={problemMessage(state.problem)}
          requestId={state.problem?.request_id}
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
        <EmptyState title={t("pm-digests-empty-title")} text={t("pm-digests-empty-text")} />
      )}
      {state.kind === "loaded" && state.items.length > 0 && (
        <>
          <p className={styles.help}>{t("pm-digests-intro")}</p>
          <DataTable
            label={t("pm-digests-title")}
            rows={state.items}
            rowKey={(digest) => digest.id}
            columns={[
              {
                id: "recipient",
                header: t("pm-digest-recipient"),
                cell: (digest) => digest.user_display_name,
              },
              {
                id: "planned",
                header: t("pm-digest-planned"),
                numeric: true,
                cell: (digest) => formatDateTime(digest.created_at, timeZone),
              },
              {
                id: "sent",
                header: t("pm-digest-sent"),
                numeric: true,
                cell: (digest) =>
                  digest.sent_at
                    ? formatDateTime(digest.sent_at, timeZone)
                    : t("pm-digest-not-sent"),
              },
              {
                id: "items",
                header: t("pm-digest-items"),
                numeric: true,
                cell: (digest) => digest.item_count,
              },
              {
                id: "status",
                header: t("pm-digest-status"),
                cell: (digest) => <DigestStatusLabel status={digest.status} />,
              },
            ]}
          />
          {state.items.some((digest) => digest.status === "unknown") && (
            <p className={styles.help}>{t("pm-digest-unknown-help")}</p>
          )}
        </>
      )}
    </section>
  );
}
