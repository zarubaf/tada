import type { Api, Workstream } from "../api/client";
import { useEventContext } from "../events/eventContext";
import { t } from "../i18n";
import { useRegisterPage } from "../registers/useRegisterPage";
import { useSession } from "../session/SessionProvider";
import { Button } from "../ui/Button";
import { type Column, DataTable } from "../ui/DataTable";
import { EmptyState } from "../ui/EmptyState";
import { LiveRegion } from "../ui/LiveRegion";
import { DirectoryGate } from "./DirectoryGate";
import { type Directory, useDirectory } from "./directory";
import styles from "./Work.module.css";
import { WorkstreamForm } from "./WorkstreamForm";

/**
 * „Arbeitsbereiche“ of an event: the register of workstreams with their leads. Event managers
 * create and change them; each member of the event reads them.
 */
export function WorkstreamsPage({ api }: { api: Api }) {
  const { event } = useEventContext();
  const { state, reload } = useDirectory(api, event.id);
  return (
    <DirectoryGate state={state} reload={reload}>
      {(directory) => (
        <Workstreams api={api} eventId={event.id} directory={directory} reloadDirectory={reload} />
      )}
    </DirectoryGate>
  );
}

function Workstreams({
  api,
  eventId,
  directory,
  reloadDirectory,
}: {
  api: Api;
  eventId: string;
  directory: Directory;
  reloadDirectory: () => Promise<boolean>;
}) {
  const session = useSession();
  // The directory holds the workstreams of the event; a save loads it again.
  const page = useRegisterPage<Workstream>(reloadDirectory);
  const { editing } = page;

  const saved = (workstream: Workstream) => {
    page.setConfirmation(t("register-saved", { name: workstream.name }));
    if (editing) {
      page.closeForm();
    }
    void reloadDirectory();
  };

  const columns: Column<Workstream>[] = [
    { id: "name", header: t("workstream-column-name"), cell: (row) => row.name },
    {
      id: "lead",
      header: t("workstream-column-lead"),
      cell: (row) => directory.nameOf(row.lead_user_id),
    },
    {
      id: "status",
      header: t("work-column-status"),
      cell: (row) => t(`workstream-status-${row.status}`),
    },
    ...(directory.isManager
      ? [
          {
            id: "actions",
            header: t("work-column-actions"),
            cell: (row: Workstream) => (
              <Button
                aria-label={t("workstream-edit-of", { name: row.name })}
                onPress={() => page.edit(row)}
              >
                {t("work-edit")}
              </Button>
            ),
          },
        ]
      : []),
  ];

  return (
    <div className={styles.page}>
      <LiveRegion kind="alert">{page.failure}</LiveRegion>
      <LiveRegion kind="status">{page.confirmation}</LiveRegion>
      <h2 ref={page.heading} tabIndex={-1} className={styles.heading}>
        {t("workstreams-title")}
      </h2>
      {directory.workstreams.length === 0 ? (
        <EmptyState title={t("workstreams-empty-title")} text={t("workstreams-empty-text")} />
      ) : (
        <DataTable
          label={t("workstreams-title")}
          columns={columns}
          rows={directory.workstreams}
          rowKey={(row) => row.id}
        />
      )}
      {directory.isManager && (
        <section className={styles.section} aria-labelledby="workstream-form-title">
          <h2
            id="workstream-form-title"
            ref={page.formHeading}
            tabIndex={-1}
            className={styles.heading}
          >
            {editing
              ? t("workstream-change-title", { name: editing.name })
              : t("workstream-create-title")}
          </h2>
          <WorkstreamForm
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
      )}
    </div>
  );
}
