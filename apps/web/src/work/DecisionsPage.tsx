import { useCallback, useRef, useState } from "react";
import { type Api, type Decision, problemMessage } from "../api/client";
import { useEventContext, useEventWritable } from "../events/eventContext";
import { formatDate } from "../facts/formatValue";
import { t } from "../i18n";
import { RegisterView } from "../registers/RegisterView";
import { type SaveFailure, saveFailure } from "../registers/saveFailure";
import { useRegister } from "../registers/useRegister";
import { useRegisterPage } from "../registers/useRegisterPage";
import { useSession } from "../session/SessionProvider";
import { Button } from "../ui/Button";
import type { Column } from "../ui/DataTable";
import { useFocusAfterCommit } from "../ui/focus";
import { LiveRegion } from "../ui/LiveRegion";
import { Redacted, redactedOr } from "../ui/Redacted";
import { DecisionForm } from "./DecisionForm";
import { DirectoryGate } from "./DirectoryGate";
import { shortWording } from "./decisionWording";
import { type Directory, useDirectory } from "./directory";
import { EvidenceSheet } from "./EvidenceSheet";
import { RevokeDialog } from "./RevokeDialog";
import { useSuccessorIds } from "./useSuccessorIds";
import styles from "./Work.module.css";
import { DecisionStatusLabel } from "./WorkStatus";
import { mayCreateKind } from "./workPermissions";

/**
 * „Entscheide“ of an event: the register of decisions. The wording never changes. A manager
 * records a decision, replaces one in force with a new one („Ersetzen“) or ends it with a reason
 * („Widerrufen“); the server says per row what the member may do (`can_end`).
 */
export function DecisionsPage({ api }: { api: Api }) {
  const { event } = useEventContext();
  const { state, reload } = useDirectory(api, event.id);
  return (
    <DirectoryGate state={state} reload={reload}>
      {(directory) => <Decisions api={api} directory={directory} />}
    </DirectoryGate>
  );
}

/** The wording in a row: the first lines, and a button that shows all of it. */
function Wording({ decision }: { decision: Decision }) {
  const [expanded, setExpanded] = useState(false);
  if (decision.redacted) {
    return <Redacted />;
  }
  const short = shortWording(decision.wording);
  return (
    <>
      <span className={styles.wording}>
        {expanded || !short.cut ? decision.wording : short.text}
      </span>
      {short.cut && (
        <Button
          variant="quiet"
          aria-expanded={expanded}
          aria-label={t(expanded ? "decision-wording-less-of" : "decision-wording-more-of", {
            id: decision.local_id,
          })}
          onPress={() => setExpanded(!expanded)}
        >
          {t(expanded ? "decision-wording-less" : "decision-wording-more")}
        </Button>
      )}
    </>
  );
}

function Decisions({ api, directory }: { api: Api; directory: Directory }) {
  const { event } = useEventContext();
  const writable = useEventWritable();
  const session = useSession();
  const fetchPage = useCallback(
    (cursor: string | undefined) =>
      api.GET("/api/v1/events/{event_id}/decisions", {
        params: { path: { event_id: event.id }, query: cursor === undefined ? {} : { cursor } },
      }),
    [api, event.id],
  );
  const register = useRegister<Decision>(fetchPage);
  const page = useRegisterPage<Decision>(register.reload);
  const [revoking, setRevoking] = useState<Decision>();
  const [showing, setShowing] = useState<Decision>();
  // The button that opened the dialog: focus returns to it when the member cancels.
  const revokeButton = useRef<HTMLElement>(null);
  const focusAfterCommit = useFocusAfterCommit();
  // The decision that the form replaces: „Ersetzen“ opens the form for the new decision. The server
  // allows it per row (`can_end`), also to a member who may not record a decision of their own.
  const { editing: replacing } = page;
  // Which form instance is on screen: a late answer must not open or close another one, also not
  // a new form of the same decision. A form opens through `openReplace` and closes through
  // `closeReplace`; a fresh version for the same form (`page.edit`) keeps the instance.
  const instance = useRef(0);
  const formInstance = instance.current;
  const openReplace = (row: Decision) => {
    if (replacing?.id !== row.id) {
      instance.current += 1;
    }
    page.edit(row);
  };
  const closeReplace = () => {
    instance.current += 1;
    page.closeForm();
  };
  const canRecord = mayCreateKind("decision", directory, session.user.id, writable);
  const items = register.state.kind === "loaded" ? register.state.items : [];

  const saved = (decision: Decision) => {
    if (replacing) {
      page.setConfirmation(
        t("decision-replaced", { id: decision.local_id, old: replacing.local_id }),
      );
      // Another replacement opened while the request ran: its form stays.
      if (instance.current === formInstance) {
        closeReplace();
      }
      // The replaced decision changed too: the list loads again.
      void register.reload();
    } else {
      page.setConfirmation(page.savedMessage(decision.local_id));
      register.append(decision);
    }
  };

  const localId = useSuccessorIds(api, event.id, items);

  /**
   * Somebody changed the decision that the form replaces. The list loads again and the form keeps
   * what the member typed; it now replaces the fresh version, if the member may still end it. The
   * form stays busy until this is done. A form that is gone or has changed meanwhile only gets the
   * message. A failed lookup leaves the old version in the form, which blocks its send.
   */
  const failedReplace = async (failure: SaveFailure, old: Decision) => {
    if (!failure.conflict) {
      page.fail(failure);
      return;
    }
    // The form that sent: a lookup that answers for a form that is gone or new only tells the message.
    const sent = formInstance;
    let message = failure.message;
    // The lookup and the reload both finish, whatever the lookup does: the reloaded row is as good
    // as the looked up one, if it is newer.
    const [one] = await Promise.allSettled([
      api.GET("/api/v1/events/{event_id}/decisions/{id}", {
        params: { path: { event_id: event.id, id: old.id } },
      }),
      register.reload(),
    ]);
    const looked = one.status === "fulfilled" ? one.value : undefined;
    const fresh =
      looked?.data ??
      register.lastLoaded().find((row) => row.id === old.id && row.version > old.version);
    if (!fresh) {
      message = looked ? saveFailure(looked).message : problemMessage(undefined);
    }
    if (instance.current !== sent) {
      page.setFailure(failure.message);
    } else if (!fresh) {
      // The button of the form goes off now (the version is refused): focus is on the heading first.
      page.formHeading.current?.focus();
      page.setFailure(message);
    } else {
      // The heading takes focus before the button of the form may go off.
      page.formHeading.current?.focus();
      page.edit(fresh);
      page.setFailure(
        fresh.can_end && fresh.status === "in-force"
          ? failure.message
          : t("decision-replace-gone", { id: fresh.local_id }),
      );
    }
  };

  const columns: Column<Decision>[] = [
    { id: "id", header: t("work-column-id"), cell: (row) => row.local_id, mono: true },
    {
      id: "wording",
      header: t("decision-column-wording"),
      cell: (row) => <Wording decision={row} />,
    },
    {
      id: "decided-on",
      header: t("decision-column-decided-on"),
      cell: (row) => formatDate(row.decided_on),
      numeric: true,
    },
    {
      id: "decided-by",
      header: t("decision-column-decided-by"),
      cell: (row) =>
        row.decided_by.map((party, index) => (
          <span key={party.id}>
            {index > 0 && ", "}
            {redactedOr(party.redacted, party.name)}
          </span>
        )),
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
      id: "status",
      header: t("work-column-status"),
      cell: (row) => {
        const successor = row.superseded_by ? localId(row.superseded_by) : undefined;
        return (
          <>
            <DecisionStatusLabel status={row.status} />
            {successor && (
              <span className={styles.statusNote}>
                {t("decision-superseded-by")}{" "}
                <a className={styles.recordLink} href={`#${successor}`}>
                  {successor}
                </a>
              </span>
            )}
            {row.revoked_reason && (
              <span className={styles.statusNote}>
                {t("decision-revoked-reason-line", { reason: row.revoked_reason })}
              </span>
            )}
          </>
        );
      },
    },
    {
      id: "actions",
      header: t("work-column-actions"),
      cell: (row) => {
        const ending = writable && row.can_end && !row.redacted;
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
            {ending && (
              <Button
                aria-label={t("decision-replace-of", { id: row.local_id })}
                onPress={() => openReplace(row)}
              >
                {t("decision-replace")}
              </Button>
            )}
            {ending && (
              <Button
                aria-label={t("decision-revoke-of", { id: row.local_id })}
                onPress={(event) => {
                  revokeButton.current = event.target as HTMLElement;
                  page.start();
                  setRevoking(row);
                }}
              >
                {t("decision-revoke")}
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
        {t("decisions-title")}
      </h2>
      <RegisterView
        register={register}
        page={page}
        label={t("decisions-title")}
        columns={columns}
        rowId={(row) => row.local_id}
        loadingLabel={t("decisions-loading")}
        empty={{
          title: t("decisions-empty-title"),
          text: canRecord ? t("decisions-empty-text") : t("decisions-empty-text-read"),
        }}
      />
      {(canRecord || replacing) && (
        <section className={styles.section} aria-labelledby="decision-form-title">
          <h2
            id="decision-form-title"
            ref={page.formHeading}
            tabIndex={-1}
            className={styles.heading}
          >
            {replacing
              ? t("decision-replace-title", { id: replacing.local_id })
              : t("decision-create-title")}
          </h2>
          <DecisionForm
            key={replacing ? replacing.id : "new"}
            api={api}
            eventId={event.id}
            directory={directory}
            userId={session.user.id}
            {...(replacing && { replaces: replacing })}
            onStart={page.start}
            onSaved={saved}
            onAnnounce={page.setConfirmation}
            onFailed={async (failure) => {
              if (replacing) {
                await failedReplace(failure, replacing);
              } else {
                page.fail(failure);
              }
            }}
            {...(replacing && { onCancel: closeReplace })}
          />
        </section>
      )}
      {revoking && (
        <RevokeDialog
          api={api}
          eventId={event.id}
          decision={revoking}
          onDone={(decision) => {
            setRevoking(undefined);
            register.replace(decision);
            page.setConfirmation(t("decision-revoked", { id: decision.local_id }));
            // The button of the row is gone: focus goes to the heading of the page.
            focusAfterCommit(() => page.heading.current);
          }}
          onFailed={(failure) => {
            setRevoking(undefined);
            // Not `page.fail`: it closes the form, and a replacement that the member is typing
            // belongs to another row.
            page.setFailure(failure.message);
            if (failure.conflict) {
              void register.reload();
            }
          }}
          onCancel={() => {
            setRevoking(undefined);
            focusAfterCommit(() => revokeButton.current);
          }}
        />
      )}
      {showing && (
        <EvidenceSheet
          title={t("work-evidence-of", { id: showing.local_id })}
          evidence={showing.evidence}
          timeZone={event.time_zone}
          versionText={(version) => t("decision-evidence-version", { version })}
          onClose={() => setShowing(undefined)}
        />
      )}
    </div>
  );
}
