import { useState } from "react";
import type { Api } from "../api/client";
import { type Failure, failureOf, invalidFailure, useWaiting } from "../api/failure";
import { t } from "../i18n";
import { useNavigate } from "../router/Router";
import { useRefreshSession } from "../session/SessionProvider";
import { Button } from "../ui/Button";
import { InlineError } from "../ui/InlineError";
import { takeFragmentToken } from "./fragment";
import { PublicPage, PublicText, ToSignInLink } from "./PublicPage";

/**
 * The page that a magic link opens. A GET shows only a confirmation: mail scanners and link
 * previews open links, and they must not use up the token (ADR 0008). The click signs in.
 */
export function MagicLinkPage({ api }: { api: Api }) {
  // The first render reads and removes the fragment. A later render must keep the token.
  const [token] = useState(takeFragmentToken);
  const [failure, setFailure] = useState<Failure | undefined>(() =>
    token ? undefined : invalidFailure(t("magic-link-invalid")),
  );
  const [busy, setBusy] = useState(false);
  const waiting = useWaiting(failure);
  const refresh = useRefreshSession();
  const navigate = useNavigate();

  const signIn = async () => {
    if (!token || busy || waiting) {
      return;
    }
    setBusy(true);
    try {
      const result = await api.POST("/api/v1/sign-in/magic-link", { body: { token } });
      if (!result.error) {
        await refresh();
        navigate("/events", { replace: true });
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
          <Button
            variant="primary"
            isPending={busy}
            isDisabled={waiting}
            onPress={() => void signIn()}
          >
            {t("magic-link-submit")}
          </Button>
        </>
      )}
      {failure && (
        <InlineError
          key={failure.id}
          message={failure.message}
          requestId={failure.requestId}
          takeFocus
        >
          {failure.final && <ToSignInLink />}
        </InlineError>
      )}
    </PublicPage>
  );
}
