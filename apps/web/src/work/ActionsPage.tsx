import { useCallback } from "react";
import type { Action, Api } from "../api/client";
import { useEventContext } from "../events/eventContext";
import { formatDate } from "../facts/formatValue";
import { t } from "../i18n";
import { useSession } from "../session/SessionProvider";
import { Button } from "../ui/Button";
import type { Column } from "../ui/DataTable";
import { LiveRegion } from "../ui/LiveRegion";
import { ActionForm } from "./ActionForm";
import { DirectoryGate } from "./DirectoryGate";
import { type Directory, useDirectory } from "./directory";
import { RegisterView } from "./RegisterView";
import { useRegister } from "./useRegister";
import { useRegisterPage } from "./useRegisterPage";
import styles from "./Work.module.css";
import { ActionStatusLabel } from "./WorkStatus";

/**
 * „Aufgaben“ of an event: the register of actions and the form that creates or changes one. The
 * owner, the lead of the workstream and an event manager can change an action; the server decides.
 */
export function ActionsPage({ api }: { api: Api }) {
  const { event } = useEventContext();
  const { state, reload } = useDirectory(api, event.id);
  return (
    <DirectoryGate state={state} reload={reload}>
      {(directory) => <Actions api={api} eventId={event.id} directory={directory} />}
    </DirectoryGate>
  );
}

function Actions({ api, eventId, directory }: { api: Api; eventId: string; directory: Directory }) {
  const session = useSession();
  const fetchPage = useCallback(
    (cursor: string | undefined) =>
      api.GET("/api/v1/events/{event_id}/actions", {
        params: { path: { event_id: eventId }, query: cursor === undefined ? {} : { cursor } },
      }),
    [api, eventId],
  );
  const register = useRegister<Action>(fetchPage);
  const page = useRegisterPage<Action>(register.reload);
  const { editing } = page;

  const saved = (action: Action) => {
    page.setConfirmation(page.savedMessage(action.local_id));
    if (editing) {
      register.replace(action);
      page.closeForm();
    } else {
      register.append(action);
    }
  };

  const columns: Column<Action>[] = [
    { id: "id", header: t("work-column-id"), cell: (row) => row.local_id, mono: true },
    { id: "title", header: t("action-column-title"), cell: (row) => row.title },
    {
      id: "owner",
      header: t("work-column-owner"),
      cell: (row) => directory.nameOf(row.owner_user_id),
    },
    {
      id: "workstream",
      header: t("work-column-workstream"),
      cell: (row) => directory.workstreams.find((w) => w.id === row.workstream_id)?.name ?? "",
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
      cell: (row) => <ActionStatusLabel status={row.status} />,
    },
    {
      id: "actions",
      header: t("work-column-actions"),
      cell: (row) =>
        directory.mayChange(row) && (
          <Button
            aria-label={t("work-edit-of", { id: row.local_id })}
            onPress={() => page.edit(row)}
          >
            {t("work-edit")}
          </Button>
        ),
    },
  ];

  return (
    <div className={styles.page}>
      <LiveRegion kind="alert">{page.failure}</LiveRegion>
      <LiveRegion kind="status">{page.confirmation}</LiveRegion>
      <h2 ref={page.heading} tabIndex={-1} className={styles.heading}>
        {t("actions-title")}
      </h2>
      <RegisterView
        register={register}
        label={t("actions-title")}
        columns={columns}
        loadingLabel={t("actions-loading")}
        empty={{ title: t("actions-empty-title"), text: t("actions-empty-text") }}
        onFailure={page.loadMoreFailed}
        onRetry={page.retry}
        retried={page.retried}
      />
      <section className={styles.section} aria-labelledby="action-form-title">
        <h2 id="action-form-title" ref={page.formHeading} tabIndex={-1} className={styles.heading}>
          {editing ? t("action-change-title", { id: editing.local_id }) : t("action-create-title")}
        </h2>
        <ActionForm
          key={editing ? `${editing.id}:${editing.version}` : "new"}
          api={api}
          eventId={eventId}
          directory={directory}
          userId={session.user.id}
          {...(editing && { record: editing })}
          onStart={page.start}
          onSaved={saved}
          onFailed={page.fail}
          {...(editing && { onCancel: () => page.closeForm() })}
        />
      </section>
    </div>
  );
}
