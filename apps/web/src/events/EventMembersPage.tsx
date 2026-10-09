import { useCallback, useEffect, useRef, useState } from "react";
import {
  type Api,
  type EventMembership,
  type EventRole,
  type Problem,
  problemMessage,
} from "../api/client";
import { failureOf } from "../api/failure";
import { t } from "../i18n";
import { useParams } from "../router/Router";
import { useSession } from "../session/SessionProvider";
import { Button } from "../ui/Button";
import { ConfirmDialog } from "../ui/ConfirmDialog";
import { type Column, DataTable } from "../ui/DataTable";
import { useFocusAfterCommit, useRetry } from "../ui/focus";
import { InlineError } from "../ui/InlineError";
import { LiveRegion } from "../ui/LiveRegion";
import { Select } from "../ui/Select";
import { Skeleton } from "../ui/Skeleton";
import styles from "./EventMembersPage.module.css";
import {
  addableMembers,
  EVENT_ROLES,
  loadOrganizationMembers,
  type OrganizationMember,
} from "./eventMembers";

interface Failure {
  message: string;
  requestId: string | undefined;
}

type State =
  | { kind: "loading" }
  | { kind: "failed"; message: string; requestId: string | undefined }
  | { kind: "loaded"; items: EventMembership[] };

const roleOptions = EVENT_ROLES.map((role) => ({ id: role, label: t(`role-${role}`) }));

/** The message of a failed action. Two codes have a text that is true only on this page. */
function actionMessage(result: { error?: Problem | undefined; response?: Response }): string {
  const { error } = result;
  if (error?.code === "invalid-transition") {
    return t("event-members-last-manager");
  }
  if (error?.code === "record-version-conflict") {
    return t("event-members-conflict");
  }
  return failureOf(result).message;
}

/**
 * „Mitglieder“ of an event: the event memberships. The server lists them only for a member who
 * manages them, so a loaded list always has the actions; the server decides each action anyway.
 */
export function EventMembersPage({ api }: { api: Api }) {
  const { eventId = "" } = useParams();
  const session = useSession();
  const [state, setState] = useState<State>({ kind: "loading" });
  // The message of the last failed action, for example the refusal to remove the last event manager.
  const [failure, setFailure] = useState<string>();
  // The result of an action that does not move focus to the result, for example an added member.
  const [confirmation, setConfirmation] = useState<string>();
  const [organization, setOrganization] = useState<OrganizationMember[]>();
  const [organizationFailure, setOrganizationFailure] = useState<Failure>();
  const [removing, setRemoving] = useState<EventMembership>();
  // A request runs: a second press does nothing.
  const [busy, setBusy] = useState(false);
  const heading = useRef<HTMLHeadingElement>(null);
  const focusAfterCommit = useFocusAfterCommit();
  const { retried, retry } = useRetry(() => heading.current);

  /** Resolves to true when the list loaded. */
  const load = useCallback(async () => {
    try {
      const { data, error } = await api.GET("/api/v1/events/{event_id}/memberships", {
        params: { path: { event_id: eventId } },
      });
      setState(
        data
          ? { kind: "loaded", items: data.items }
          : { kind: "failed", message: problemMessage(error), requestId: error?.request_id },
      );
      return data !== undefined;
    } catch {
      setState({ kind: "failed", message: problemMessage(undefined), requestId: undefined });
      return false;
    }
  }, [api, eventId]);

  useEffect(() => {
    void load();
  }, [load]);

  const items = state.kind === "loaded" ? state.items : [];
  const loaded = state.kind === "loaded";

  /** Resolves to true when the members of the organization loaded. */
  const loadOrganization = useCallback(async () => {
    try {
      const result = await loadOrganizationMembers(api);
      if ("members" in result) {
        setOrganization(result.members);
        setOrganizationFailure(undefined);
        return true;
      }
      setOrganizationFailure({
        message: problemMessage(result.error),
        requestId: result.error?.request_id,
      });
    } catch {
      setOrganizationFailure({ message: problemMessage(undefined), requestId: undefined });
    }
    return false;
  }, [api]);

  useEffect(() => {
    if (loaded) {
      void loadOrganization();
    }
  }, [loaded, loadOrganization]);

  const replace = (items: EventMembership[], changed: EventMembership) =>
    items.map((item) => (item.user_id === changed.user_id ? changed : item));

  /** A failed action: a version conflict also loads the list again. */
  const fail = (result: { error?: Problem | undefined; response?: Response }) => {
    const { error } = result;
    setFailure(actionMessage(result));
    if (error?.code === "record-version-conflict") {
      void load();
    }
  };

  const changeRole = async (item: EventMembership, role: EventRole) => {
    if (role === item.event_role || busy) {
      return;
    }
    setFailure(undefined);
    setBusy(true);
    try {
      const { data, error, response } = await api.POST(
        "/api/v1/events/{event_id}/memberships/{user_id}/change-role",
        {
          params: { path: { event_id: eventId, user_id: item.user_id } },
          body: { event_role: role, expected_version: item.version },
        },
      );
      if (data) {
        setState((current) =>
          current.kind === "loaded"
            ? { kind: "loaded", items: replace(current.items, data) }
            : current,
        );
      } else {
        fail({ error, response });
      }
    } catch {
      fail({});
    }
    setBusy(false);
  };

  const remove = async (item: EventMembership) => {
    if (busy) {
      return;
    }
    setFailure(undefined);
    setBusy(true);
    try {
      const { response, error } = await api.POST(
        "/api/v1/events/{event_id}/memberships/{user_id}/remove",
        {
          params: { path: { event_id: eventId, user_id: item.user_id } },
          body: { expected_version: item.version },
        },
      );
      if (response.ok) {
        // The pressed button leaves with its row: focus goes to the heading of the list.
        focusAfterCommit(() => heading.current);
        setState((current) =>
          current.kind === "loaded"
            ? { kind: "loaded", items: current.items.filter((i) => i.user_id !== item.user_id) }
            : current,
        );
      } else {
        fail({ error, response });
      }
    } catch {
      fail({});
    }
    setBusy(false);
    setRemoving(undefined);
  };

  const columns: Column<EventMembership>[] = [
    { id: "name", header: t("event-members-column-name"), cell: (item) => item.display_name },
    {
      id: "role",
      header: t("event-members-column-role"),
      cell: (item) => (
        <Select
          label={t("event-members-role-of", { name: item.display_name })}
          labelHidden
          options={roleOptions}
          value={item.event_role}
          onChange={(role) => void changeRole(item, role as EventRole)}
        />
      ),
    },
    {
      id: "actions",
      header: t("event-members-column-actions"),
      cell: (item) => (
        <Button
          variant="danger"
          aria-label={t("event-members-remove-of", { name: item.display_name })}
          onPress={() => setRemoving(item)}
        >
          {t("event-members-remove")}
        </Button>
      ),
    },
  ];

  if (state.kind === "loading") {
    return (
      <div className={styles.skeleton} role="status" aria-label={t("event-members-loading")}>
        <Skeleton />
        <Skeleton />
        <Skeleton />
      </div>
    );
  }
  if (state.kind === "failed") {
    return (
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
    );
  }

  return (
    <div className={styles.members}>
      <h2 ref={heading} tabIndex={-1} className={styles.heading}>
        {t("event-members-title")}
      </h2>
      <LiveRegion kind="alert">{failure}</LiveRegion>
      <LiveRegion kind="status">{confirmation}</LiveRegion>
      <DataTable
        label={t("event-members-title")}
        columns={columns}
        rows={items}
        rowKey={(item) => item.user_id}
      />
      <AddMember
        api={api}
        eventId={eventId}
        candidates={organization && addableMembers(organization, items)}
        organizationFailure={organizationFailure}
        onRetry={() => {
          setOrganizationFailure(undefined);
          return loadOrganization();
        }}
        onAdded={(added) => {
          setConfirmation(t("event-members-added", { name: added.display_name }));
          setState((current) =>
            current.kind === "loaded"
              ? { kind: "loaded", items: [...current.items, added] }
              : current,
          );
        }}
        onFailed={fail}
        onStart={() => {
          setFailure(undefined);
          setConfirmation(undefined);
        }}
      />
      <ConfirmDialog
        isOpen={removing !== undefined}
        title={t("event-members-remove-title", { name: removing?.display_name ?? "" })}
        text={t("event-members-remove-text", { name: removing?.display_name ?? "" })}
        warning={
          removing?.user_id === session.user.id && removing.event_role === "event-manager"
            ? t("event-members-remove-self")
            : undefined
        }
        confirmLabel={t("event-members-remove")}
        cancelLabel={t("event-members-remove-cancel")}
        isPending={busy}
        onConfirm={() => removing && void remove(removing)}
        onCancel={() => !busy && setRemoving(undefined)}
      />
    </div>
  );
}

function AddMember({
  api,
  eventId,
  candidates,
  organizationFailure,
  onRetry,
  onAdded,
  onFailed,
  onStart,
}: {
  api: Api;
  eventId: string;
  /** Nothing while the organization members load. */
  candidates: OrganizationMember[] | undefined;
  organizationFailure: Failure | undefined;
  /** Loads the members of the organization again; resolves to true when they loaded. */
  onRetry: () => Promise<boolean>;
  onAdded: (added: EventMembership) => void;
  onFailed: (result: { error?: Problem | undefined; response?: Response }) => void;
  onStart: () => void;
}) {
  const [userId, setUserId] = useState<string>();
  const [role, setRole] = useState<EventRole>("event-contributor");
  const [pending, setPending] = useState(false);
  const section = useRef<HTMLElement>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  const focusAfterCommit = useFocusAfterCommit();
  const { retried, retry } = useRetry(() => heading.current);

  if (organizationFailure) {
    return (
      <InlineError
        message={organizationFailure.message}
        requestId={organizationFailure.requestId}
        onRetry={() => retry(onRetry)}
        announce={retried ? "focus" : "alert"}
      />
    );
  }
  if (!candidates) {
    return <Skeleton />;
  }

  const add = async () => {
    if (!userId || pending) {
      return;
    }
    onStart();
    setPending(true);
    try {
      const { data, error, response } = await api.POST("/api/v1/events/{event_id}/memberships", {
        params: { path: { event_id: eventId } },
        body: { user_id: userId, event_role: role },
      });
      if (data) {
        setUserId(undefined);
        onAdded(data);
        // The button is disabled again, so focus moves to the first control of the form, or to the
        // heading if no member is left to add.
        focusAfterCommit(
          () =>
            section.current?.querySelector<HTMLElement>("button") ??
            section.current?.querySelector<HTMLElement>("h2"),
        );
      } else {
        onFailed({ error, response });
      }
    } catch {
      onFailed({});
    }
    setPending(false);
  };

  return (
    <section ref={section} className={styles.add} aria-labelledby="members-add-title">
      <h2 id="members-add-title" ref={heading} tabIndex={-1} className={styles.heading}>
        {t("event-members-add-title")}
      </h2>
      {candidates.length === 0 ? (
        <p>{t("event-members-add-none")}</p>
      ) : (
        <div className={styles.addForm}>
          <Select
            label={t("event-members-add-member")}
            placeholder={t("event-members-add-placeholder")}
            options={candidates.map((member) => ({
              id: member.user_id,
              label: member.display_name,
            }))}
            value={userId}
            onChange={setUserId}
          />
          <Select
            label={t("event-members-column-role")}
            options={roleOptions}
            value={role}
            onChange={(id) => setRole(id as EventRole)}
          />
          <Button
            variant="primary"
            isDisabled={!userId}
            isPending={pending}
            onPress={() => void add()}
          >
            {t("event-members-add-submit")}
          </Button>
        </div>
      )}
    </section>
  );
}
