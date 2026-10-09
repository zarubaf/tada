import { useCallback, useEffect, useRef, useState } from "react";
import {
  type Api,
  type Invitation,
  type Member,
  type Problem,
  problemMessage,
} from "../api/client";
import { failureOf } from "../api/failure";
import { LOCALE, t } from "../i18n";
import { useNavigate } from "../router/Router";
import { CHOOSE_ORGANIZATION_PATH } from "../session/paths";
import { useSession } from "../session/SessionProvider";
import { Button } from "../ui/Button";
import { ConfirmDialog } from "../ui/ConfirmDialog";
import { type Column, DataTable } from "../ui/DataTable";
import { EmptyState } from "../ui/EmptyState";
import { useFocusAfterCommit, useRetry } from "../ui/focus";
import { InlineError } from "../ui/InlineError";
import { LiveRegion } from "../ui/LiveRegion";
import { PageTitle } from "../ui/Page";
import { Skeleton } from "../ui/Skeleton";
import { InviteMemberForm } from "./InviteMemberForm";
import styles from "./MembersPage.module.css";
import { canManage, canRemove, invitableRoles } from "./roles";

const createdFormat = new Intl.DateTimeFormat(LOCALE, { dateStyle: "medium" });

interface Loadable<T> {
  kind: "loading" | "failed" | "loaded";
  items: T[];
  nextCursor?: string | undefined;
  message?: string;
  requestId?: string | undefined;
}

const loading = <T,>(): Loadable<T> => ({ kind: "loading", items: [] });

/** What the member confirms: the removal of a member or the revocation of an invitation. */
type Confirming =
  | { kind: "member"; item: Member }
  | { kind: "sessions"; item: Member }
  | { kind: "invitation"; item: Invitation };

/**
 * „Mitglieder“ in the settings: the members, and for owners and admins the pending invitations
 * and the invitation form. The server decides each action; the page hides what would fail.
 */
export function MembersPage({ api }: { api: Api }) {
  const session = useSession();
  const role = session.organization?.role;
  const manages = canManage(role);
  const [members, setMembers] = useState<Loadable<Member>>(loading());
  const [invitations, setInvitations] = useState<Loadable<Invitation>>(loading());
  // The next page of members loads: the loaded rows stay while it loads or fails.
  const [loadingMore, setLoadingMore] = useState(false);
  const navigate = useNavigate();
  // The message of the last failed action. Both live regions are in the page from the start.
  const [failure, setFailure] = useState<string>();
  const [confirmation, setConfirmation] = useState<string>();
  const [confirming, setConfirming] = useState<Confirming>();
  // A request runs: a second press does nothing.
  const [busy, setBusy] = useState(false);
  const membersHeading = useRef<HTMLHeadingElement>(null);
  const invitationsHeading = useRef<HTMLHeadingElement>(null);
  const alert = useRef<HTMLParagraphElement>(null);
  const focusAfterCommit = useFocusAfterCommit();
  const membersRetry = useRetry(() => membersHeading.current);
  const invitationsRetry = useRetry(() => invitationsHeading.current);

  /** The first page. A version conflict and a retry load the list again from here. */
  const loadMembers = useCallback(async () => {
    try {
      const { data, error } = await api.GET("/api/v1/members");
      setMembers(
        data
          ? { kind: "loaded", items: data.items, nextCursor: data.next_cursor ?? undefined }
          : {
              kind: "failed",
              items: [],
              message: problemMessage(error),
              requestId: error?.request_id,
            },
      );
      setLoadingMore(false);
      return data !== undefined;
    } catch {
      setMembers({ kind: "failed", items: [], message: problemMessage(undefined) });
      return false;
    }
  }, [api]);

  const loadInvitations = useCallback(async () => {
    try {
      const { data, error } = await api.GET("/api/v1/invitations");
      setInvitations(
        data
          ? { kind: "loaded", items: data.items }
          : {
              kind: "failed",
              items: [],
              message: problemMessage(error),
              requestId: error?.request_id,
            },
      );
      return data !== undefined;
    } catch {
      setInvitations({ kind: "failed", items: [], message: problemMessage(undefined) });
      return false;
    }
  }, [api]);

  useEffect(() => {
    void loadMembers();
  }, [loadMembers]);

  useEffect(() => {
    if (manages) {
      void loadInvitations();
    }
  }, [manages, loadInvitations]);

  /** A failed action: a version conflict also loads the list again. */
  const fail = (result: { error?: Problem | undefined; response?: Response }) => {
    const code = result.error?.code;
    setFailure(
      code === "invalid-transition" && confirming?.kind === "member"
        ? t("members-remove-last")
        : code === "record-version-conflict"
          ? t("members-conflict")
          : failureOf(result).message,
    );
    // The failure may be far above the button that the member pressed.
    alert.current?.scrollIntoView?.({ block: "nearest" });
    if (code === "record-version-conflict") {
      void loadMembers();
    }
    if (code === "not-found" && confirming?.kind === "invitation") {
      // The person accepted the invitation in the meantime.
      void loadInvitations();
    }
  };

  /** The next page: the loaded rows stay, and a retry continues from the same cursor. */
  const loadMore = async () => {
    if (members.kind !== "loaded" || members.nextCursor === undefined || loadingMore) {
      return;
    }
    setLoadingMore(true);
    setFailure(undefined);
    setConfirmation(undefined);
    try {
      const { data, error } = await api.GET("/api/v1/members", {
        params: { query: { cursor: members.nextCursor } },
      });
      if (data) {
        const nextCursor = data.next_cursor ?? undefined;
        setMembers((current) => ({
          ...current,
          items: [...current.items, ...data.items],
          nextCursor,
        }));
        setConfirmation(t("members-loaded-more"));
        if (nextCursor === undefined) {
          // The button leaves: focus goes to the heading of the list.
          focusAfterCommit(() => membersHeading.current);
        }
      } else {
        // The button stays and keeps focus: the alert region announces the failure.
        fail({ error });
      }
    } catch {
      fail({});
    }
    setLoadingMore(false);
  };

  const confirm = async () => {
    if (busy || !confirming) {
      return;
    }
    setFailure(undefined);
    setConfirmation(undefined);
    setBusy(true);
    try {
      if (confirming.kind === "sessions") {
        const { item } = confirming;
        const { response, error } = await api.POST("/api/v1/members/{user_id}/sessions/end", {
          params: { path: { user_id: item.user_id } },
        });
        if (response.ok) {
          setConfirmation(t("members-end-sessions-ended", { name: item.display_name }));
        } else {
          fail({ error, response });
        }
      } else if (confirming.kind === "member") {
        const { item } = confirming;
        const { response, error } = await api.POST("/api/v1/members/{user_id}/remove", {
          params: { path: { user_id: item.user_id } },
          body: { expected_version: item.version },
        });
        if (response.ok) {
          if (item.user_id === session.user.id) {
            // The member left: the organization context is gone, so no manager mode stays.
            setBusy(false);
            setConfirming(undefined);
            await session.refresh();
            navigate(CHOOSE_ORGANIZATION_PATH, { replace: true });
            return;
          }
          focusAfterCommit(() => membersHeading.current);
          setMembers((current) => ({
            ...current,
            items: current.items.filter((m) => m.user_id !== item.user_id),
          }));
        } else {
          fail({ error, response });
        }
      } else {
        const { item } = confirming;
        const { response, error } = await api.POST("/api/v1/invitations/{invitation_id}/revoke", {
          params: { path: { invitation_id: item.id } },
        });
        if (response.ok) {
          focusAfterCommit(() => invitationsHeading.current);
          setInvitations((current) => ({
            ...current,
            items: current.items.filter((i) => i.id !== item.id),
          }));
        } else {
          fail({ error, response });
        }
      }
    } catch {
      fail({});
    }
    setBusy(false);
    setConfirming(undefined);
  };

  const memberColumns: Column<Member>[] = [
    { id: "name", header: t("members-column-name"), cell: (member) => member.display_name },
    ...(manages
      ? [{ id: "email", header: t("members-column-email"), cell: (member: Member) => member.email }]
      : []),
    { id: "role", header: t("members-column-role"), cell: (member) => t(`role-${member.role}`) },
    {
      id: "actions",
      header: t("members-column-actions"),
      cell: (member) =>
        member.user_id === session.user.id ? (
          // Each member can leave.
          <Button variant="danger" onPress={() => setConfirming({ kind: "member", item: member })}>
            {t("members-leave")}
          </Button>
        ) : (
          canRemove(role, member.role) && (
            <div className={styles.rowActions}>
              {/* The remedy for a stolen session, also where a removal is not possible. */}
              <Button
                aria-label={t("members-end-sessions-of", { name: member.display_name })}
                onPress={() => setConfirming({ kind: "sessions", item: member })}
              >
                {t("members-end-sessions")}
              </Button>
              <Button
                variant="danger"
                aria-label={t("members-remove-of", { name: member.display_name })}
                onPress={() => setConfirming({ kind: "member", item: member })}
              >
                {t("members-remove")}
              </Button>
            </div>
          )
        ),
    },
  ];

  const invitationColumns: Column<Invitation>[] = [
    { id: "name", header: t("members-column-name"), cell: (invitation) => invitation.display_name },
    { id: "email", header: t("members-column-email"), cell: (invitation) => invitation.email },
    {
      id: "role",
      header: t("members-column-role"),
      cell: (invitation) => t(`role-${invitation.role}`),
    },
    {
      id: "invited",
      header: t("members-column-invited"),
      cell: (invitation) => (
        <time dateTime={invitation.created_at}>
          {createdFormat.format(new Date(invitation.created_at))}
        </time>
      ),
      numeric: true,
    },
    {
      id: "actions",
      header: t("members-column-actions"),
      // An admin cannot revoke an owner invitation: the same rule as for a removal (ADR 0056).
      cell: (invitation) =>
        canRemove(role, invitation.role) && (
          <Button
            variant="danger"
            aria-label={t("invitations-revoke-of", { name: invitation.display_name })}
            onPress={() => setConfirming({ kind: "invitation", item: invitation })}
          >
            {t("invitations-revoke")}
          </Button>
        ),
    },
  ];

  const retryMembers = () =>
    membersRetry.retry(() => {
      setMembers(loading());
      return loadMembers();
    });
  const retryInvitations = () =>
    invitationsRetry.retry(() => {
      setInvitations(loading());
      return loadInvitations();
    });

  const revoking = confirming?.kind === "invitation";
  const name = confirming?.item.display_name ?? "";
  const leaving = confirming?.kind === "member" && confirming.item.user_id === session.user.id;

  const dialog = revoking
    ? {
        title: t("invitations-revoke-title"),
        text: t("invitations-revoke-text", { name }),
        confirm: t("invitations-revoke"),
      }
    : confirming?.kind === "sessions"
      ? {
          title: t("members-end-sessions-title"),
          text: t("members-end-sessions-text", { name }),
          confirm: t("members-end-sessions"),
        }
      : leaving
        ? {
            title: t("members-leave-title"),
            text: t("members-leave-text"),
            confirm: t("members-leave"),
          }
        : {
            title: t("members-remove-title"),
            text: t("members-remove-text", { name }),
            confirm: t("members-remove"),
          };

  return (
    <div className={styles.page}>
      <LiveRegion ref={alert} kind="alert">
        {failure}
      </LiveRegion>
      <LiveRegion kind="status">{confirmation}</LiveRegion>

      <section className={styles.section} aria-labelledby="members-title">
        <PageTitle id="members-title" ref={membersHeading}>
          {t("members-title")}
        </PageTitle>
        {members.kind === "loading" && (
          <div className={styles.skeleton} role="status" aria-label={t("members-loading")}>
            <Skeleton />
            <Skeleton />
            <Skeleton />
          </div>
        )}
        {members.kind === "failed" && (
          <InlineError
            message={members.message ?? ""}
            requestId={members.requestId}
            onRetry={retryMembers}
            announce={membersRetry.retried ? "focus" : "alert"}
          />
        )}
        {members.kind === "loaded" && (
          <>
            <DataTable
              label={t("members-title")}
              columns={memberColumns}
              rows={members.items}
              rowKey={(member) => member.user_id}
            />
            {members.nextCursor !== undefined && (
              <Button isPending={loadingMore} onPress={() => void loadMore()}>
                {t("members-load-more")}
              </Button>
            )}
          </>
        )}
      </section>

      {manages && (
        <section className={styles.section} aria-labelledby="invitations-title">
          <h2
            id="invitations-title"
            ref={invitationsHeading}
            tabIndex={-1}
            className={styles.heading}
          >
            {t("invitations-title")}
          </h2>
          {invitations.kind === "loading" && (
            <div className={styles.skeleton} role="status" aria-label={t("invitations-loading")}>
              <Skeleton />
              <Skeleton />
            </div>
          )}
          {invitations.kind === "failed" && (
            <InlineError
              message={invitations.message ?? ""}
              requestId={invitations.requestId}
              onRetry={retryInvitations}
              announce={invitationsRetry.retried ? "focus" : "alert"}
            />
          )}
          {invitations.kind === "loaded" && invitations.items.length === 0 && (
            <EmptyState title={t("invitations-empty-title")} text={t("invitations-empty-text")} />
          )}
          {invitations.kind === "loaded" && invitations.items.length > 0 && (
            <DataTable
              label={t("invitations-title")}
              columns={invitationColumns}
              rows={invitations.items}
              rowKey={(invitation) => invitation.id}
            />
          )}
        </section>
      )}

      {manages && (
        <InviteMemberForm
          api={api}
          roles={invitableRoles(role)}
          onStart={() => {
            setFailure(undefined);
            setConfirmation(undefined);
          }}
          onFailed={setFailure}
          onInvited={(invitation) => {
            setConfirmation(t("invite-sent", { name: invitation.display_name }));
            // A list that is not loaded yet or failed gets the new invitation with its next load.
            if (invitations.kind === "loaded") {
              setInvitations({
                ...invitations,
                items: [...invitations.items.filter((i) => i.id !== invitation.id), invitation],
              });
            } else {
              void loadInvitations();
            }
          }}
        />
      )}

      <ConfirmDialog
        isOpen={confirming !== undefined}
        title={dialog.title}
        text={dialog.text}
        warning={leaving ? t("members-leave-warning") : undefined}
        confirmLabel={dialog.confirm}
        cancelLabel={t("members-remove-cancel")}
        isPending={busy}
        onConfirm={() => void confirm()}
        onCancel={() => !busy && setConfirming(undefined)}
      />
    </div>
  );
}
