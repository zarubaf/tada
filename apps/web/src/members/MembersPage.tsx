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
import { useSession } from "../session/SessionProvider";
import { Button } from "../ui/Button";
import { ConfirmDialog } from "../ui/ConfirmDialog";
import { type Column, DataTable } from "../ui/DataTable";
import { EmptyState } from "../ui/EmptyState";
import { InlineError } from "../ui/InlineError";
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
  /** Counts the retries, so that a failure after a retry takes focus. */
  attempts: number;
}

const loading = <T,>(attempts = 0): Loadable<T> => ({ kind: "loading", items: [], attempts });

/** What the member confirms: the removal of a member or the revocation of an invitation. */
type Confirming = { kind: "member"; item: Member } | { kind: "invitation"; item: Invitation };

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
  // The message of the last failed action. Both live regions are in the page from the start.
  const [failure, setFailure] = useState<string>();
  const [confirmation, setConfirmation] = useState<string>();
  const [confirming, setConfirming] = useState<Confirming>();
  // A request runs: a second press does nothing.
  const [busy, setBusy] = useState(false);
  const membersHeading = useRef<HTMLHeadingElement>(null);
  const invitationsHeading = useRef<HTMLHeadingElement>(null);
  const focusAfterClose = useRef<"member" | "invitation">(undefined);

  // The pressed button leaves with its row. When the dialog has closed and given its focus back,
  // focus goes to the heading of the list.
  useEffect(() => {
    if (confirming === undefined && focusAfterClose.current) {
      const heading = focusAfterClose.current === "member" ? membersHeading : invitationsHeading;
      focusAfterClose.current = undefined;
      setTimeout(() => heading.current?.focus(), 0);
    }
  }, [confirming]);

  const loadMembers = useCallback(
    async (cursor: string | undefined, previous: Member[], attempts: number) => {
      try {
        const { data, error } = await api.GET("/api/v1/members", {
          params: { query: cursor === undefined ? {} : { cursor } },
        });
        setMembers(
          data
            ? {
                kind: "loaded",
                items: [...previous, ...data.items],
                nextCursor: data.next_cursor ?? undefined,
                attempts,
              }
            : {
                kind: "failed",
                items: previous,
                message: problemMessage(error),
                requestId: error?.request_id,
                attempts,
              },
        );
      } catch {
        setMembers({
          kind: "failed",
          items: previous,
          message: problemMessage(undefined),
          attempts,
        });
      }
    },
    [api],
  );

  const loadInvitations = useCallback(
    async (attempts: number) => {
      try {
        const { data, error } = await api.GET("/api/v1/invitations");
        setInvitations(
          data
            ? { kind: "loaded", items: data.items, attempts }
            : {
                kind: "failed",
                items: [],
                message: problemMessage(error),
                requestId: error?.request_id,
                attempts,
              },
        );
      } catch {
        setInvitations({ kind: "failed", items: [], message: problemMessage(undefined), attempts });
      }
    },
    [api],
  );

  useEffect(() => {
    void loadMembers(undefined, [], 0);
  }, [loadMembers]);

  useEffect(() => {
    if (manages) {
      void loadInvitations(0);
    }
  }, [manages, loadInvitations]);

  /** A failed action: a version conflict also loads the list again. */
  const fail = (result: { error?: Problem | undefined; response?: Response }) => {
    setFailure(failureOf(result).message);
    if (result.error?.code === "record-version-conflict") {
      void loadMembers(undefined, [], members.attempts);
    }
  };

  const confirm = async () => {
    if (busy || !confirming) {
      return;
    }
    setFailure(undefined);
    setConfirmation(undefined);
    setBusy(true);
    try {
      if (confirming.kind === "member") {
        const { item } = confirming;
        const { response, error } = await api.POST("/api/v1/members/{user_id}/remove", {
          params: { path: { user_id: item.user_id } },
          body: { expected_version: item.version },
        });
        if (response.ok) {
          focusAfterClose.current = "member";
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
          focusAfterClose.current = "invitation";
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
    ...(manages
      ? [
          {
            id: "actions",
            header: t("members-column-actions"),
            cell: (member: Member) =>
              canRemove(role, member.role) && (
                <Button
                  aria-label={t("members-remove-of", { name: member.display_name })}
                  onPress={() => setConfirming({ kind: "member", item: member })}
                >
                  {t("members-remove")}
                </Button>
              ),
          },
        ]
      : []),
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
      cell: (invitation) => (
        <Button
          aria-label={t("invitations-revoke-of", { name: invitation.display_name })}
          onPress={() => setConfirming({ kind: "invitation", item: invitation })}
        >
          {t("invitations-revoke")}
        </Button>
      ),
    },
  ];

  const retryMembers = () => {
    const attempts = members.attempts + 1;
    setMembers(loading(attempts));
    void loadMembers(undefined, [], attempts);
  };
  const retryInvitations = () => {
    const attempts = invitations.attempts + 1;
    setInvitations(loading(attempts));
    void loadInvitations(attempts);
  };

  const revoking = confirming?.kind === "invitation";
  const name = confirming?.item.display_name ?? "";

  return (
    <main id="main" className={styles.page}>
      {/* Live regions that are always in the page: a text that is set later is announced. */}
      <p className={styles.failure} role="alert">
        {failure}
      </p>
      <p className={styles.confirmation} role="status">
        {confirmation}
      </p>

      <section className={styles.section} aria-labelledby="members-title">
        <h1 id="members-title" ref={membersHeading} tabIndex={-1} className={styles.title}>
          {t("members-title")}
        </h1>
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
            takeFocus={members.attempts > 0}
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
              <Button
                onPress={() => {
                  const cursor = members.nextCursor;
                  setMembers({ ...members, nextCursor: undefined });
                  void loadMembers(cursor, members.items, members.attempts);
                }}
              >
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
              takeFocus={invitations.attempts > 0}
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
            setInvitations((current) => ({
              ...current,
              items: [...current.items.filter((i) => i.id !== invitation.id), invitation],
            }));
          }}
        />
      )}

      <ConfirmDialog
        isOpen={confirming !== undefined}
        title={revoking ? t("invitations-revoke-title") : t("members-remove-title")}
        text={
          revoking ? t("invitations-revoke-text", { name }) : t("members-remove-text", { name })
        }
        confirmLabel={revoking ? t("invitations-revoke") : t("members-remove")}
        cancelLabel={t("members-remove-cancel")}
        isPending={busy}
        onConfirm={() => void confirm()}
        onCancel={() => !busy && setConfirming(undefined)}
      />
    </main>
  );
}
