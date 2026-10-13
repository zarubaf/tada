import { IconAlertTriangle } from "@tabler/icons-react";
import { useCallback, useRef, useState } from "react";
import {
  type Action,
  type Api,
  type Milestone,
  type MilestoneStatus,
  type MilestoneWarning,
  problemMessage,
} from "../api/client";
import { useEventContext, useEventWritable } from "../events/eventContext";
import { formatDate } from "../facts/formatValue";
import { t } from "../i18n";
import { RegisterView } from "../registers/RegisterView";
import { saveFailure } from "../registers/saveFailure";
import { useRegister } from "../registers/useRegister";
import { useRegisterPage } from "../registers/useRegisterPage";
import { useSession } from "../session/SessionProvider";
import { Button } from "../ui/Button";
import type { Column } from "../ui/DataTable";
import { useFocusAfterCommit } from "../ui/focus";
import { LiveRegion } from "../ui/LiveRegion";
import { redactedOr } from "../ui/Redacted";
import { DirectoryGate } from "./DirectoryGate";
import { type Directory, useDirectory } from "./directory";
import { EvidenceSheet } from "./EvidenceSheet";
import { MilestoneForm } from "./MilestoneForm";
import styles from "./Work.module.css";
import { MilestoneStatusLabel } from "./WorkStatus";

/** The statuses of an action that still has work to do. */
const OPEN_ACTION_STATUSES: readonly Action["status"][] = ["open", "in-progress", "blocked"];

/**
 * The status change that a row offers: „reached“ for an open milestone, „open“ again for a reached
 * or canceled one. The form offers every status that the server allows, also „canceled“.
 */
const ROW_STATUSES = {
  reached: { label: "milestone-reach", of: "milestone-reach-of" },
  open: { label: "milestone-reopen", of: "milestone-reopen-of" },
} as const;

function rowStatus(milestone: Milestone): keyof typeof ROW_STATUSES | undefined {
  return (["reached", "open"] as const).find((status) => milestone.next_statuses.includes(status));
}

/**
 * „Meilensteine“ of an event: the register of milestones with the count of the open actions that
 * lead to each and the warnings of the server. A row reaches a milestone or opens it again; the
 * form sets any status that the server allows (`next_statuses`).
 */
export function MilestonesPage({ api }: { api: Api }) {
  const { event } = useEventContext();
  // The actions give the count of open actions and the names of the late ones.
  const { state, reload } = useDirectory(api, event.id, { actions: true });
  return (
    <DirectoryGate state={state} reload={reload}>
      {(directory) => <Milestones api={api} directory={directory} />}
    </DirectoryGate>
  );
}

function Milestones({ api, directory }: { api: Api; directory: Directory }) {
  const { event } = useEventContext();
  const writable = useEventWritable();
  const session = useSession();
  const fetchPage = useCallback(
    (cursor: string | undefined) =>
      api.GET("/api/v1/events/{event_id}/milestones", {
        params: { path: { event_id: event.id }, query: cursor === undefined ? {} : { cursor } },
      }),
    [api, event.id],
  );
  const register = useRegister<Milestone>(fetchPage);
  const page = useRegisterPage<Milestone>(register.reload);
  const [showing, setShowing] = useState<Milestone>();
  const [changing, setChanging] = useState<string>();
  const statusButtons = useRef(new Map<string, HTMLButtonElement>());
  const focusAfterCommit = useFocusAfterCommit();
  const { editing } = page;

  const saved = (milestone: Milestone) => {
    page.setConfirmation(page.savedMessage(milestone.local_id));
    if (editing) {
      register.replace(milestone);
      page.closeForm();
    } else {
      register.append(milestone);
    }
  };

  const setStatus = async (row: Milestone, status: MilestoneStatus) => {
    page.start();
    if (changing) {
      return;
    }
    setChanging(row.id);
    try {
      const result = await api.PATCH("/api/v1/events/{event_id}/milestones/{id}", {
        params: { path: { event_id: event.id, id: row.id } },
        body: { status, expected_version: row.version },
      });
      if (result.data) {
        register.replace(result.data);
        page.setConfirmation(
          t("milestone-status-changed", {
            id: result.data.local_id,
            status: t(`milestone-status-${result.data.status}`),
          }),
        );
        // The pressed button gives way to the opposite change of the row, which takes focus.
        focusAfterCommit(() => statusButtons.current.get(row.id) ?? page.heading.current);
      } else {
        page.fail(saveFailure(result));
      }
    } catch {
      page.fail({ message: problemMessage(undefined), conflict: false });
    }
    setChanging(undefined);
  };

  const localIds = new Map(directory.actions.map((action) => [action.id, action.local_id]));
  const openActions = (milestone: Milestone) =>
    directory.actions.filter(
      (action) =>
        action.milestone_id === milestone.id && OPEN_ACTION_STATUSES.includes(action.status),
    ).length;

  const warningText = (warning: MilestoneWarning) => {
    if (warning.kind === "overdue") {
      return t("milestone-warning-overdue");
    }
    const id = localIds.get(warning.action_id);
    return id ? t("milestone-warning-late-action", { id }) : t("milestone-warning-late-unknown");
  };

  const columns: Column<Milestone>[] = [
    { id: "id", header: t("work-column-id"), cell: (row) => row.local_id, mono: true },
    {
      id: "title",
      header: t("milestone-column-title"),
      cell: (row) => (
        <>
          {redactedOr(row.redacted, row.title)}
          {row.warnings.length > 0 && (
            <ul className={styles.warnings}>
              {row.warnings.map((warning) => (
                <li
                  key={warning.kind === "overdue" ? "overdue" : warning.action_id}
                  className={styles.warning}
                >
                  <IconAlertTriangle
                    className={styles.warningIcon}
                    size={16}
                    stroke={1.5}
                    aria-hidden="true"
                  />
                  {warningText(warning)}
                </li>
              ))}
            </ul>
          )}
        </>
      ),
    },
    {
      id: "date",
      header: t("milestone-column-date"),
      cell: (row) => formatDate(row.date),
      numeric: true,
    },
    {
      id: "owner",
      header: t("work-column-owner"),
      cell: (row) => directory.nameOf(row.owner_user_id),
    },
    {
      id: "workstream",
      header: t("work-column-workstream"),
      cell: (row) => {
        const stream = directory.workstreams.find((w) => w.id === row.workstream_id);
        return stream ? redactedOr(stream.redacted, stream.name) : "";
      },
    },
    {
      id: "open-actions",
      header: t("milestone-column-open-actions"),
      cell: (row) => String(openActions(row)),
      numeric: true,
    },
    {
      id: "status",
      header: t("work-column-status"),
      cell: (row) => <MilestoneStatusLabel status={row.status} />,
    },
    {
      id: "actions",
      header: t("work-column-actions"),
      cell: (row) => {
        const changeable = writable && row.can_change && !row.redacted;
        const status = rowStatus(row);
        return (
          <span className={styles.rowActions}>
            {row.evidence.length > 0 && (
              <Button
                aria-label={t("work-evidence-of", { id: row.local_id })}
                onPress={() => setShowing(row)}
              >
                {t("work-evidence")}
              </Button>
            )}
            {changeable && (
              <Button
                aria-label={t("work-edit-of", { id: row.local_id })}
                onPress={() => page.edit(row)}
              >
                {t("work-edit")}
              </Button>
            )}
            {changeable && status && (
              <Button
                ref={(element) => {
                  if (element) {
                    statusButtons.current.set(row.id, element);
                  } else {
                    statusButtons.current.delete(row.id);
                  }
                }}
                aria-label={t(ROW_STATUSES[status].of, { id: row.local_id })}
                isPending={changing === row.id}
                onPress={() => void setStatus(row, status)}
              >
                {t(ROW_STATUSES[status].label)}
              </Button>
            )}
          </span>
        );
      },
    },
  ];

  return (
    <div className={styles.page}>
      <LiveRegion kind="alert">{page.failure}</LiveRegion>
      <LiveRegion kind="status">{page.confirmation}</LiveRegion>
      <h2 ref={page.heading} tabIndex={-1} className={styles.heading}>
        {t("milestones-title")}
      </h2>
      <RegisterView
        register={register}
        page={page}
        label={t("milestones-title")}
        columns={columns}
        rowId={(row) => row.local_id}
        loadingLabel={t("milestones-loading")}
        empty={{ title: t("milestones-empty-title"), text: t("milestones-empty-text") }}
      />
      {writable && (
        <section className={styles.section} aria-labelledby="milestone-form-title">
          <h2
            id="milestone-form-title"
            ref={page.formHeading}
            tabIndex={-1}
            className={styles.heading}
          >
            {editing
              ? t("milestone-change-title", { id: editing.local_id })
              : t("milestone-create-title")}
          </h2>
          <MilestoneForm
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
      )}
      {showing && (
        <EvidenceSheet
          title={t("work-evidence-of", { id: showing.local_id })}
          evidence={showing.evidence}
          timeZone={event.time_zone}
          versionText={(version) => t("milestone-evidence-version", { version })}
          onClose={() => setShowing(undefined)}
        />
      )}
    </div>
  );
}
