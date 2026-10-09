import { useCallback, useState } from "react";
import type { Api, Commitment } from "../api/client";
import { useEventContext } from "../events/eventContext";
import { formatDate } from "../facts/formatValue";
import { t } from "../i18n";
import { RegisterView } from "../registers/RegisterView";
import { useRegister } from "../registers/useRegister";
import { useRegisterPage } from "../registers/useRegisterPage";
import { useSession } from "../session/SessionProvider";
import { Button } from "../ui/Button";
import type { Column } from "../ui/DataTable";
import { LiveRegion } from "../ui/LiveRegion";
import { CommitmentEvidence } from "./CommitmentEvidence";
import { CommitmentForm } from "./CommitmentForm";
import { DirectoryGate } from "./DirectoryGate";
import { type Directory, useDirectory } from "./directory";
import { MakeFirmDialog } from "./MakeFirmDialog";
import styles from "./Work.module.css";
import { CommitmentStatusLabel } from "./WorkStatus";

/**
 * „Zusagen“ of an event: the register of commitments with the condition, the promisor and the
 * evidence. A conditional commitment becomes firm only with „Verbindlich machen“ and a reason.
 */
export function CommitmentsPage({ api }: { api: Api }) {
  const { event } = useEventContext();
  const { state, reload } = useDirectory(api, event.id);
  return (
    <DirectoryGate state={state} reload={reload}>
      {(directory) => <Commitments api={api} directory={directory} />}
    </DirectoryGate>
  );
}

function Commitments({ api, directory }: { api: Api; directory: Directory }) {
  const { event } = useEventContext();
  const session = useSession();
  const fetchPage = useCallback(
    (cursor: string | undefined) =>
      api.GET("/api/v1/events/{event_id}/commitments", {
        params: { path: { event_id: event.id }, query: cursor === undefined ? {} : { cursor } },
      }),
    [api, event.id],
  );
  const register = useRegister<Commitment>(fetchPage);
  const page = useRegisterPage<Commitment>(register.reload);
  const [firming, setFirming] = useState<Commitment>();
  const [showing, setShowing] = useState<Commitment>();
  const { editing } = page;

  const saved = (commitment: Commitment) => {
    page.setConfirmation(page.savedMessage(commitment.local_id));
    if (editing) {
      register.replace(commitment);
      page.closeForm();
    } else {
      register.append(commitment);
    }
  };

  const columns: Column<Commitment>[] = [
    { id: "id", header: t("work-column-id"), cell: (row) => row.local_id, mono: true },
    {
      id: "text",
      header: t("commitment-column-text"),
      cell: (row) => (
        <>
          {row.text}
          {row.condition && (
            <span className={styles.condition}>
              {t("commitment-condition-line", { condition: row.condition })}
            </span>
          )}
          {row.firm_reason && (
            <span className={styles.condition}>
              {t("commitment-firm-reason-line", { reason: row.firm_reason })}
            </span>
          )}
        </>
      ),
    },
    { id: "promisor", header: t("commitment-column-promisor"), cell: (row) => row.promisor.name },
    {
      id: "owner",
      header: t("work-column-owner"),
      cell: (row) => directory.nameOf(row.owner_user_id),
    },
    {
      id: "due",
      header: t("work-column-due"),
      cell: (row) => (row.due_date ? formatDate(row.due_date) : ""),
      numeric: true,
    },
    {
      id: "status",
      header: t("work-column-status"),
      cell: (row) => <CommitmentStatusLabel status={row.status} />,
    },
    {
      id: "actions",
      header: t("work-column-actions"),
      cell: (row) => (
        <span className={styles.rowActions}>
          {row.evidence.length > 0 && (
            <Button
              aria-label={t("commitment-evidence-of", { id: row.local_id })}
              onPress={() => setShowing(row)}
            >
              {t("commitment-evidence")}
            </Button>
          )}
          {row.can_change && (
            <Button
              aria-label={t("work-edit-of", { id: row.local_id })}
              onPress={() => page.edit(row)}
            >
              {t("work-edit")}
            </Button>
          )}
          {row.can_make_firm && (
            <Button
              aria-label={t("make-firm-of", { id: row.local_id })}
              onPress={() => {
                page.start();
                setFirming(row);
              }}
            >
              {t("make-firm")}
            </Button>
          )}
        </span>
      ),
    },
  ];

  return (
    <div className={styles.page}>
      <LiveRegion kind="alert">{page.failure}</LiveRegion>
      <LiveRegion kind="status">{page.confirmation}</LiveRegion>
      <h2 ref={page.heading} tabIndex={-1} className={styles.heading}>
        {t("commitments-title")}
      </h2>
      <RegisterView
        register={register}
        page={page}
        label={t("commitments-title")}
        columns={columns}
        loadingLabel={t("commitments-loading")}
        empty={{ title: t("commitments-empty-title"), text: t("commitments-empty-text") }}
      />
      <section className={styles.section} aria-labelledby="commitment-form-title">
        <h2
          id="commitment-form-title"
          ref={page.formHeading}
          tabIndex={-1}
          className={styles.heading}
        >
          {editing
            ? t("commitment-change-title", { id: editing.local_id })
            : t("commitment-create-title")}
        </h2>
        <CommitmentForm
          key={editing ? `${editing.id}:${editing.version}` : "new"}
          api={api}
          eventId={event.id}
          directory={directory}
          userId={session.user.id}
          {...(editing && { record: editing })}
          onStart={page.start}
          onSaved={saved}
          onFailed={page.fail}
          {...(editing && { onCancel: () => page.closeForm() })}
        />
      </section>
      {firming && (
        <MakeFirmDialog
          api={api}
          eventId={event.id}
          commitment={firming}
          onDone={(commitment) => {
            setFirming(undefined);
            register.replace(commitment);
            page.setConfirmation(t("make-firm-done", { id: commitment.local_id }));
          }}
          onFailed={(failure) => {
            setFirming(undefined);
            page.fail(failure);
          }}
          onCancel={() => setFirming(undefined)}
        />
      )}
      {showing && (
        <CommitmentEvidence
          title={t("commitment-evidence-of", { id: showing.local_id })}
          evidence={showing.evidence}
          timeZone={event.time_zone}
          onClose={() => setShowing(undefined)}
        />
      )}
    </div>
  );
}
