import { IconLock } from "@tabler/icons-react";
import { useRef, useState } from "react";
import type { Api, Event } from "../api/client";
import { type Failure, failureOf } from "../api/failure";
import { formatDay } from "../facts/formatValue";
import { t } from "../i18n";
import { canManage } from "../members/roles";
import { useSession } from "../session/SessionProvider";
import { Banner } from "../ui/Banner";
import { Button } from "../ui/Button";
import { useFocusAfterCommit } from "../ui/focus";
import { InlineError } from "../ui/InlineError";
import { TaskDialog, TaskDialogActions } from "../ui/TaskDialog";
import { useManagesEvent } from "./useManagesEvent";

export interface CloseEventProps {
  api: Api;
  event: Event;
  /** The event after a successful close or reopen. */
  onChanged: (event: Event) => void;
  /** The event changed elsewhere: the page loads it again. */
  onStale: () => void;
}

/**
 * Closing and reopening of an event (ADR 0071). An active event shows „Anlass abschliessen“ to an
 * event manager, an owner and an admin. A closed event shows the banner with its date; an owner or
 * an admin reopens it, unless retention redacted it. Both changes ask for a confirmation. The banner or the button that
 * replaces the control takes focus; a failure shows as an alert.
 */
export function CloseEvent({ api, event, onChanged, onStale }: CloseEventProps) {
  const session = useSession();
  const manages = useManagesEvent(api, event.id);
  const isAdmin = canManage(session.organization?.role);
  const closed = event.status === "closed";
  const [asking, setAsking] = useState(false);
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<Pick<Failure, "message" | "requestId">>();
  const banner = useRef<HTMLDivElement>(null);
  const closeButton = useRef<HTMLButtonElement>(null);
  const reopenButton = useRef<HTMLButtonElement>(null);
  const focusAfterCommit = useFocusAfterCommit();

  const change = async () => {
    if (busy) {
      return;
    }
    setBusy(true);
    setFailure(undefined);
    const path = { event_id: event.id };
    const body = { expected_version: event.version };
    try {
      const result = closed
        ? await api.POST("/api/v1/events/{event_id}/reopen", { params: { path }, body })
        : await api.POST("/api/v1/events/{event_id}/close", { params: { path }, body });
      if (result.data) {
        onChanged(result.data);
        // The control that opened the dialog leaves with the state: focus goes to its successor.
        focusAfterCommit(() => (closed ? closeButton.current : banner.current));
      } else {
        focusAfterCommit(() => (closed ? reopenButton.current : closeButton.current));
        setFailure(failureOf(result));
        if (result.error?.code !== "forbidden") {
          onStale();
        }
      }
    } catch {
      focusAfterCommit(() => (closed ? reopenButton.current : closeButton.current));
      setFailure(failureOf({}));
    }
    setAsking(false);
    setBusy(false);
  };

  /** Closes the dialog; focus returns to the control that opened it. */
  const cancel = () => {
    if (!busy) {
      setAsking(false);
      focusAfterCommit(() => (closed ? reopenButton.current : closeButton.current));
    }
  };
  const canClose = !closed && (manages || isAdmin);
  const canReopen = closed && isAdmin && !event.redacted;
  return (
    <>
      {failure && <InlineError message={failure.message} requestId={failure.requestId} />}
      {closed && (
        <Banner
          ref={banner}
          icon={IconLock}
          action={
            canReopen && (
              <Button ref={reopenButton} onPress={() => setAsking(true)}>
                {t("event-reopen")}
              </Button>
            )
          }
        >
          <strong>
            {event.closed_at
              ? t("event-closed-on", { date: formatDay(event.closed_at, event.time_zone) })
              : t("event-closed")}
          </strong>
          <span>{t(event.redacted ? "event-closed-redacted" : "event-closed-note")}</span>
        </Banner>
      )}
      {canClose && (
        <div>
          <Button ref={closeButton} onPress={() => setAsking(true)}>
            {t("event-close")}
          </Button>
        </div>
      )}
      {asking && (
        <TaskDialog
          title={t(closed ? "event-reopen-title" : "event-close-title")}
          onCancel={cancel}
        >
          <p>{t(closed ? "event-reopen-text" : "event-close-text")}</p>
          <TaskDialogActions>
            <Button onPress={cancel}>{t("event-close-cancel")}</Button>
            <Button isPending={busy} onPress={() => void change()}>
              {t(closed ? "event-reopen" : "event-close")}
            </Button>
          </TaskDialogActions>
        </TaskDialog>
      )}
    </>
  );
}
