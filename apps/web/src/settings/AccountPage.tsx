import { useState } from "react";
import type { Api } from "../api/client";
import { failureOf } from "../api/failure";
import { t } from "../i18n";
import { useNavigate } from "../router/Router";
import { SIGN_IN_PATH } from "../session/paths";
import { useSession } from "../session/SessionProvider";
import { Button } from "../ui/Button";
import { ConfirmDialog } from "../ui/ConfirmDialog";
import { LiveRegion } from "../ui/LiveRegion";
import { PageTitle } from "../ui/Page";
import styles from "./OrganizationPage.module.css";

/**
 * „Konto“ in the settings: the own sessions of the member. „Überall abmelden“ ends all sessions of
 * the member, in each organization, the current one included: the remedy for a lost device or a
 * stolen session. The member then signs in again with a magic link.
 */
export function AccountPage({ api }: { api: Api }) {
  const session = useSession();
  const navigate = useNavigate();
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<string>();

  const signOutEverywhere = async () => {
    if (busy) {
      return;
    }
    setFailure(undefined);
    setBusy(true);
    try {
      const result = await api.POST("/api/v1/session/sign-out-everywhere");
      if (result.response.ok) {
        // The session of this page ended too: the session loads again as signed out.
        await session.refresh();
        navigate(SIGN_IN_PATH, { replace: true });
        return;
      }
      setFailure(failureOf(result).message);
    } catch {
      setFailure(failureOf({}).message);
    }
    setBusy(false);
    setConfirming(false);
  };

  return (
    <div className={styles.page}>
      <LiveRegion kind="alert">{failure}</LiveRegion>
      <section className={styles.section} aria-labelledby="account-title">
        <PageTitle id="account-title">{t("account-title")}</PageTitle>
        <h2>{t("account-sessions-title")}</h2>
        <p>{t("account-sessions-text")}</p>
        <div>
          <Button variant="danger" onPress={() => setConfirming(true)}>
            {t("account-sign-out-everywhere")}
          </Button>
        </div>
      </section>
      <ConfirmDialog
        isOpen={confirming}
        title={t("account-sign-out-everywhere-title")}
        text={t("account-sign-out-everywhere-text")}
        confirmLabel={t("account-sign-out-everywhere")}
        cancelLabel={t("members-remove-cancel")}
        isPending={busy}
        onConfirm={() => void signOutEverywhere()}
        onCancel={() => !busy && setConfirming(false)}
      />
    </div>
  );
}
