import { useCallback, useEffect, useState } from "react";
import {
  type Api,
  type EventMembership,
  type EventRole,
  type Problem,
  problemMessage,
} from "../api/client";
import { t } from "../i18n";
import { useParams } from "../router/Router";
import { useSession } from "../session/SessionProvider";
import { Button } from "../ui/Button";
import { type Column, DataTable } from "../ui/DataTable";
import { InlineError } from "../ui/InlineError";
import { Select } from "../ui/Select";
import { Skeleton } from "../ui/Skeleton";
import styles from "./EventMembersPage.module.css";
import {
  addableMembers,
  canManageMembers,
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

/**
 * „Mitglieder“ of an event: the event memberships. Only a member who manages them sees the
 * actions; the server decides anyway.
 */
export function EventMembersPage({ api }: { api: Api }) {
  const { eventId = "" } = useParams();
  const session = useSession();
  const [state, setState] = useState<State>({ kind: "loading" });
  // The message of the last failed action, for example the refusal to remove the last event manager.
  const [failure, setFailure] = useState<string>();
  const [organization, setOrganization] = useState<OrganizationMember[]>();
  const [organizationFailure, setOrganizationFailure] = useState<Failure>();

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
    } catch {
      setState({ kind: "failed", message: problemMessage(undefined), requestId: undefined });
    }
  }, [api, eventId]);

  useEffect(() => {
    void load();
  }, [load]);

  const items = state.kind === "loaded" ? state.items : [];
  const canManage =
    state.kind === "loaded" &&
    canManageMembers(session.organization?.role ?? "member", session.user.id, items);

  const loadOrganization = useCallback(async () => {
    try {
      const result = await loadOrganizationMembers(api);
      if ("members" in result) {
        setOrganization(result.members);
        setOrganizationFailure(undefined);
      } else {
        setOrganizationFailure({
          message: problemMessage(result.error),
          requestId: result.error?.request_id,
        });
      }
    } catch {
      setOrganizationFailure({ message: problemMessage(undefined), requestId: undefined });
    }
  }, [api]);

  useEffect(() => {
    if (canManage) {
      void loadOrganization();
    }
  }, [canManage, loadOrganization]);

  const replace = (items: EventMembership[], changed: EventMembership) =>
    items.map((item) => (item.user_id === changed.user_id ? changed : item));

  /** A failed action: a version conflict also loads the list again. */
  const fail = (error: Problem | undefined) => {
    setFailure(problemMessage(error));
    if (error?.code === "record-version-conflict") {
      void load();
    }
  };

  const changeRole = async (item: EventMembership, role: EventRole) => {
    if (role === item.event_role) {
      return;
    }
    setFailure(undefined);
    try {
      const { data, error } = await api.POST(
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
        fail(error);
      }
    } catch {
      fail(undefined);
    }
  };

  const remove = async (item: EventMembership) => {
    setFailure(undefined);
    try {
      const { response, error } = await api.POST(
        "/api/v1/events/{event_id}/memberships/{user_id}/remove",
        {
          params: { path: { event_id: eventId, user_id: item.user_id } },
          body: { expected_version: item.version },
        },
      );
      if (response.ok) {
        setState((current) =>
          current.kind === "loaded"
            ? { kind: "loaded", items: current.items.filter((i) => i.user_id !== item.user_id) }
            : current,
        );
      } else {
        fail(error);
      }
    } catch {
      fail(undefined);
    }
  };

  const columns: Column<EventMembership>[] = [
    { id: "name", header: t("event-members-column-name"), cell: (item) => item.display_name },
    {
      id: "role",
      header: t("event-members-column-role"),
      cell: (item) =>
        canManage ? (
          <Select
            label={t("event-members-role-of", { name: item.display_name })}
            labelHidden
            options={roleOptions}
            value={item.event_role}
            onChange={(role) => void changeRole(item, role as EventRole)}
          />
        ) : (
          t(`role-${item.event_role}`)
        ),
    },
    ...(canManage
      ? [
          {
            id: "actions",
            header: t("event-members-column-actions"),
            cell: (item: EventMembership) => (
              <Button
                aria-label={t("event-members-remove-of", { name: item.display_name })}
                onPress={() => void remove(item)}
              >
                {t("event-members-remove")}
              </Button>
            ),
          },
        ]
      : []),
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
        onRetry={() => {
          setState({ kind: "loading" });
          void load();
        }}
      />
    );
  }

  return (
    <div className={styles.members}>
      <h2 className={styles.heading}>{t("event-members-title")}</h2>
      {failure && (
        <p className={styles.failure} role="alert">
          {failure}
        </p>
      )}
      <DataTable
        label={t("event-members-title")}
        columns={columns}
        rows={items}
        rowKey={(item) => item.user_id}
      />
      {canManage && (
        <AddMember
          api={api}
          eventId={eventId}
          candidates={organization && addableMembers(organization, items)}
          organizationFailure={organizationFailure}
          onRetry={() => void loadOrganization()}
          onAdded={(added) =>
            setState((current) =>
              current.kind === "loaded"
                ? { kind: "loaded", items: [...current.items, added] }
                : current,
            )
          }
          onFailed={fail}
        />
      )}
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
}: {
  api: Api;
  eventId: string;
  /** Nothing while the organization members load. */
  candidates: OrganizationMember[] | undefined;
  organizationFailure: Failure | undefined;
  onRetry: () => void;
  onAdded: (added: EventMembership) => void;
  onFailed: (error: Problem | undefined) => void;
}) {
  const [userId, setUserId] = useState<string>();
  const [role, setRole] = useState<EventRole>("event-contributor");

  if (organizationFailure) {
    return (
      <InlineError
        message={organizationFailure.message}
        requestId={organizationFailure.requestId}
        onRetry={onRetry}
      />
    );
  }
  if (!candidates) {
    return <Skeleton />;
  }

  const add = async () => {
    if (!userId) {
      return;
    }
    try {
      const { data, error } = await api.POST("/api/v1/events/{event_id}/memberships", {
        params: { path: { event_id: eventId } },
        body: { user_id: userId, event_role: role },
      });
      if (data) {
        setUserId(undefined);
        onAdded(data);
      } else {
        onFailed(error);
      }
    } catch {
      onFailed(undefined);
    }
  };

  return (
    <section className={styles.add} aria-labelledby="members-add-title">
      <h2 id="members-add-title" className={styles.heading}>
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
          <Button variant="primary" isDisabled={!userId} onPress={() => void add()}>
            {t("event-members-add-submit")}
          </Button>
        </div>
      )}
    </section>
  );
}
