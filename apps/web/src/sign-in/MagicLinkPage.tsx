import { useState } from "react";
import { type Api, problemMessage } from "../api/client";
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
  const [failure, setFailure] = useState<{ message: string; requestId?: string } | null>(
    token ? null : { message: t("magic-link-invalid") },
  );
  const [busy, setBusy] = useState(false);
  const refresh = useRefreshSession();
  const navigate = useNavigate();

  const signIn = async () => {
    if (!token) {
      return;
    }
    setBusy(true);
    try {
      const { error } = await api.POST("/api/v1/sign-in/magic-link", { body: { token } });
      if (!error) {
        await refresh();
        navigate("/events", { replace: true });
        return;
      }
      setFailure({
        message: error.status < 500 ? t("magic-link-invalid") : problemMessage(error),
        requestId: error.status < 500 ? undefined : error.request_id,
      });
    } catch {
      setFailure({ message: problemMessage(undefined) });
    }
    setBusy(false);
  };

  return (
    <PublicPage title={t("magic-link-title")}>
      {failure ? (
        <InlineError message={failure.message} requestId={failure.requestId}>
          <ToSignInLink />
        </InlineError>
      ) : (
        <>
          <PublicText>{t("magic-link-text")}</PublicText>
          <Button variant="primary" isDisabled={busy} onPress={() => void signIn()}>
            {t("magic-link-submit")}
          </Button>
        </>
      )}
    </PublicPage>
  );
}
