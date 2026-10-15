import { useRef, useState } from "react";
import type { Api, Workstream } from "../api/client";
import { t } from "../i18n";
import type { SaveFailure } from "../registers/saveFailure";
import { useSession } from "../session/SessionProvider";
import { Button } from "../ui/Button";
import { EmptyState } from "../ui/EmptyState";
import { useFocusAfterCommit } from "../ui/focus";
import { LiveRegion } from "../ui/LiveRegion";
import { redactedOr } from "../ui/Redacted";
import { Sheet } from "../ui/Sheet";
import type { Directory } from "./directory";
import { WorkstreamForm } from "./WorkstreamForm";
import styles from "./WorkstreamsDialog.module.css";

export interface WorkstreamsDialogProps {
  api: Api;
  eventId: string;
  directory: Directory;
  /** False for a closed event, which is read-only for everybody (ADR 0071). */
  writable: boolean;
  /** Loads the directory again, so that the page shows the change at once. */
  reloadDirectory: () => Promise<boolean>;
  onClose: () => void;
}

/**
 * „Bereiche verwalten“ (ADR 0082): the workstreams of the event with their lead and status. Each
 * member reads the list; an event manager creates, renames, closes and reopens them (ADR 0067), not
 * in a closed event. It is a sheet over the „Arbeit“ page and is open while it is mounted.
 */
export function WorkstreamsDialog({
  api,
  eventId,
  directory,
  writable,
  reloadDirectory,
  onClose,
}: WorkstreamsDialogProps) {
  const userId = useSession().user.id;
  const canChange = directory.isManager && writable;
  const [editing, setEditing] = useState<Workstream>();
  const [failure, setFailure] = useState<string>();
  const [confirmation, setConfirmation] = useState<string>();
  const list = useRef<HTMLUListElement>(null);
  const formHeading = useRef<HTMLHeadingElement>(null);
  const focusAfterCommit = useFocusAfterCommit();

  const start = () => {
    setFailure(undefined);
    setConfirmation(undefined);
  };
  const edit = (workstream: Workstream) => {
    start();
    setEditing(workstream);
    focusAfterCommit(() => formHeading.current);
  };
  const saved = (workstream: Workstream) => {
    setConfirmation(t("register-saved", { name: workstream.name }));
    if (editing) {
      setEditing(undefined);
      // Focus goes back to the row that the member changed.
      focusAfterCommit(() =>
        list.current?.querySelector<HTMLElement>(`[data-edit="${CSS.escape(workstream.id)}"]`),
      );
    }
    void reloadDirectory();
  };
  const failed = ({ message, conflict }: SaveFailure) => {
    setFailure(message);
    if (conflict) {
      setEditing(undefined);
      void reloadDirectory();
    }
  };

  return (
    <Sheet title={t("workstreams-manage-title")} onClose={onClose}>
      <div className={styles.content}>
        <LiveRegion kind="alert">{failure}</LiveRegion>
        <LiveRegion kind="status">{confirmation}</LiveRegion>
        {directory.workstreams.length === 0 ? (
          <EmptyState
            title={t("workstreams-manage-empty-title")}
            text={t(
              canChange ? "workstreams-manage-empty-text-manager" : "workstreams-manage-empty-text",
            )}
          />
        ) : (
          <ul ref={list} className={styles.list} aria-label={t("workstreams-manage-list")}>
            {directory.workstreams.map((row) => (
              <li key={row.id} className={styles.row}>
                <div className={styles.text}>
                  <span className={styles.name}>{redactedOr(row.redacted, row.name)}</span>
                  <span className={styles.meta}>
                    <span>{directory.nameOf(row.lead_user_id)}</span>
                    <span className={styles.status}>
                      <span aria-hidden="true">· </span>
                      {t(`workstream-status-${row.status}`)}
                    </span>
                  </span>
                </div>
                {canChange && !row.redacted && (
                  <Button
                    data-edit={row.id}
                    aria-label={t("workstream-edit-of", { name: row.name })}
                    onPress={() => edit(row)}
                  >
                    {t("work-edit")}
                  </Button>
                )}
              </li>
            ))}
          </ul>
        )}
        {canChange && (
          <section className={styles.form} aria-labelledby="workstream-form-title">
            <h3
              id="workstream-form-title"
              ref={formHeading}
              tabIndex={-1}
              className={styles.formTitle}
            >
              {editing
                ? t("workstream-change-title", { name: editing.name })
                : t("workstreams-manage-create")}
            </h3>
            <WorkstreamForm
              key={editing ? `${editing.id}:${editing.version}` : "new"}
              api={api}
              eventId={eventId}
              directory={directory}
              userId={userId}
              {...(editing && { record: editing })}
              onStart={start}
              onSaved={saved}
              onFailed={failed}
              {...(editing && {
                onCancel: () => {
                  setEditing(undefined);
                  focusAfterCommit(() =>
                    list.current?.querySelector<HTMLElement>(
                      `[data-edit="${CSS.escape(editing.id)}"]`,
                    ),
                  );
                },
              })}
            />
          </section>
        )}
      </div>
    </Sheet>
  );
}
