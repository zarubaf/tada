import { useEffect, useState } from "react";
import type { Api } from "../api/client";
import { type Failure, failureOf, invalidFailure, useWaiting } from "../api/failure";
import { t } from "../i18n";
import { useNavigate } from "../router/Router";
import { useRefreshSession } from "../session/SessionProvider";
import { Button } from "../ui/Button";
import { InlineError } from "../ui/InlineError";
import { LiveRegion } from "../ui/LiveRegion";
import { takeFragmentToken } from "./fragment";
import { PublicPage, PublicText, ToSignInLink } from "./PublicPage";

/**
 * The page that a magic link opens. A GET shows only a confirmation: mail scanners and link
 * previews open links, and they must not use up the token (ADR 0008). The click signs in.
 * The page names the masked address of the account, so that a person who got the link of someone
 * else sees it before the click (login CSRF).
 */
export function MagicLinkPage({ api }: { api: Api }) {
  // The first render reads and removes the fragment. A later render must keep the token.
  const [token] = useState(takeFragmentToken);
  const [failure, setFailure] = useState<Failure | undefined>(() =>
    token ? undefined : invalidFailure(t("magic-link-invalid")),
  );
  const [busy, setBusy] = useState(false);
  // The masked address of the account of the link, when the preview loaded.
  const [emailHint, setEmailHint] = useState<string>();
  const waiting = useWaiting(failure);

  useEffect(() => {
    if (!token) {
      return;
    }
    let current = true;
    api
      .POST("/api/v1/sign-in/magic-link/preview", { body: { token } })
      .then((result) => {
        if (!current) {
          return;
        }
        if (result.data) {
          setEmailHint(result.data.email_hint);
        } else if (result.error?.code === "unauthenticated") {
          setFailure(invalidFailure(t("magic-link-invalid")));
        }
        // Another failure leaves the button: the click decides.
      })
      .catch(() => {});
    return () => {
      current = false;
    };
  }, [api, token]);
  const refresh = useRefreshSession();
  const navigate = useNavigate();

  const signIn = async () => {
    if (!token || busy || waiting) {
      return;
    }
    // Clear the message first, so that an identical one is announced again.
    setFailure(undefined);
    setBusy(true);
    try {
      const result = await api.POST("/api/v1/sign-in/magic-link", { body: { token } });
      if (!result.error) {
        await refresh();
        navigate("/", { replace: true });
        return;
      }
      setFailure(failureOf(result, t("magic-link-invalid")));
    } catch {
      setFailure(failureOf({}));
    }
    setBusy(false);
  };

  return (
    <PublicPage title={t("magic-link-title")}>
      {!failure?.final && (
        <>
          <PublicText>{t("magic-link-text")}</PublicText>
          {emailHint && <PublicText>{t("magic-link-account", { email: emailHint })}</PublicText>}
          <Button variant="primary" isPending={busy || waiting} onPress={() => void signIn()}>
            {t("magic-link-submit")}
          </Button>
        </>
      )}
      {/* The button stays after a failure that is not final, so it keeps focus. */}
      <LiveRegion kind="alert" visuallyHidden>
        {failure && !failure.final ? failure.message : undefined}
      </LiveRegion>
      {failure && (
        <InlineError
          key={failure.id}
          message={failure.message}
          requestId={failure.requestId}
          announce={failure.final ? "focus" : "none"}
        >
          {failure.final && <ToSignInLink />}
        </InlineError>
      )}
    </PublicPage>
  );
}
