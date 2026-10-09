import { useState } from "react";
import type { Api } from "../api/client";
import { type Failure, failureOf, useWaiting } from "../api/failure";
import { t } from "../i18n";
import { useNavigate } from "../router/Router";
import { PublicPage, PublicText } from "../sign-in/PublicPage";
import { Button } from "../ui/Button";
import { ChoiceButton } from "../ui/ChoiceButton";
import { EmptyState } from "../ui/EmptyState";
import { InlineError } from "../ui/InlineError";
import { LiveRegion } from "../ui/LiveRegion";
import styles from "./ChooseOrganizationPage.module.css";
import { useRefreshSession, useSession } from "./SessionProvider";

/**
 * „Organisation wählen“: a member with several memberships, or none chosen yet, picks the
 * organization of the session (ADR 0056). The shell switcher makes the same call. The page has no
 * shell, so it has its own way to sign out, for example for a member without a membership.
 */
export function ChooseOrganizationPage({ api }: { api: Api }) {
  const { memberships, signOut } = useSession();
  const refresh = useRefreshSession();
  const navigate = useNavigate();
  const [failure, setFailure] = useState<Failure>();
  // The message of a failed sign-out. The button stays, so the alert region announces it.
  const [signOutFailure, setSignOutFailure] = useState<string>();
  const [busy, setBusy] = useState(false);
  const waiting = useWaiting(failure);

  const choose = async (organizationId: string) => {
    if (busy || waiting) {
      return;
    }
    // Clear the message first, so that an identical one is announced again.
    setFailure(undefined);
    setBusy(true);
    setSignOutFailure(undefined);
    try {
      const result = await api.POST("/api/v1/session/organization", {
        body: { organization_id: organizationId },
      });
      if (!result.error) {
        await refresh();
        navigate("/", { replace: true });
        return;
      }
      setFailure(failureOf(result));
    } catch {
      setFailure(failureOf({}));
    }
    setBusy(false);
  };

  return (
    <PublicPage title={t("choose-organization-title")}>
      {memberships.length === 0 ? (
        <EmptyState
          title={t("choose-organization-empty-title")}
          text={t("choose-organization-empty-text")}
        />
      ) : (
        <>
          <PublicText>{t("choose-organization-text")}</PublicText>
          <ul className={styles.list}>
            {memberships.map((membership) => (
              <li key={membership.organization_id}>
                <ChoiceButton
                  title={membership.name}
                  detail={t(`role-${membership.role}`)}
                  isPending={busy || waiting}
                  onPress={() => void choose(membership.organization_id)}
                />
              </li>
            ))}
          </ul>
        </>
      )}
      <div>
        <Button
          onPress={() => {
            setFailure(undefined);
            void signOut().then(setSignOutFailure);
          }}
        >
          {t("sign-out")}
        </Button>
      </div>
      {/* The buttons stay, so the pressed one keeps focus. */}
      <LiveRegion kind="alert" visuallyHidden>
        {failure?.message ?? signOutFailure}
      </LiveRegion>
      {failure && (
        <InlineError
          key={failure.id}
          message={failure.message}
          requestId={failure.requestId}
          announce="none"
        />
      )}
      {signOutFailure && !failure && <InlineError message={signOutFailure} announce="none" />}
    </PublicPage>
  );
}
