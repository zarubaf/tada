import { useEffect, useState } from "react";
import type { Api, InvitationPreview } from "../api/client";
import { t } from "../i18n";
import { useNavigate } from "../router/Router";
import { useRefreshSession } from "../session/SessionProvider";
import { Button } from "../ui/Button";
import { InlineError } from "../ui/InlineError";
import { type Failure, failureOf, invalidFailure, useWaiting } from "./failure";
import { takeFragmentToken } from "./fragment";
import { PublicPage, PublicText, ToSignInLink } from "./PublicPage";

type Preview = { kind: "loading" } | { kind: "ready"; preview: InvitationPreview };

/**
 * The page that an invitation link opens. The preview shows the organization and the role without
 * using the token. Only the click accepts the invitation (ADR 0008, ADR 0056).
 */
export function InvitationPage({ api }: { api: Api }) {
  // The first render reads and removes the fragment. A later render must keep the token.
  const [token] = useState(takeFragmentToken);
  const [preview, setPreview] = useState<Preview>({ kind: "loading" });
  // The failure of the preview or of the click. A final failure replaces the page content.
  const [failure, setFailure] = useState<Failure | undefined>(() =>
    token ? undefined : invalidFailure(t("invitation-invalid")),
  );
  const [attempt, setAttempt] = useState(0);
  const [busy, setBusy] = useState(false);
  const waiting = useWaiting(failure);
  const refresh = useRefreshSession();
  const navigate = useNavigate();

  // biome-ignore lint/correctness/useExhaustiveDependencies: `attempt` loads the preview again
  useEffect(() => {
    if (!token) {
      return;
    }
    let current = true;
    api
      .POST("/api/v1/invitations/preview", { body: { token } })
      .then((result) => {
        if (!current) {
          return;
        }
        if (result.data) {
          setFailure(undefined);
          setPreview({ kind: "ready", preview: result.data });
        } else {
          setFailure(failureOf(result, t("invitation-invalid")));
        }
      })
      .catch(() => current && setFailure(failureOf({})));
    return () => {
      current = false;
    };
  }, [api, token, attempt]);

  const accept = async () => {
    if (!token || busy || waiting) {
      return;
    }
    setBusy(true);
    try {
      const result = await api.POST("/api/v1/invitations/accept", { body: { token } });
      if (!result.error) {
        await refresh();
        navigate("/events", { replace: true });
        return;
      }
      setFailure(failureOf(result, t("invitation-invalid")));
    } catch {
      setFailure(failureOf({}));
    }
    setBusy(false);
  };

  return (
    <PublicPage title={t("invitation-title")}>
      {/* The live region exists before its text, so that screen readers announce the change. */}
      <p role="status">{preview.kind === "loading" && !failure ? t("invitation-loading") : ""}</p>
      {preview.kind === "ready" && !failure?.final && (
        <>
          <PublicText>
            {t("invitation-text", {
              organization: preview.preview.organization_name,
              role: t(`role-${preview.preview.role}`),
            })}
          </PublicText>
          <Button
            variant="primary"
            isPending={busy}
            isDisabled={waiting}
            onPress={() => void accept()}
          >
            {t("invitation-accept")}
          </Button>
        </>
      )}
      {failure && (
        <InlineError
          key={failure.id}
          message={failure.message}
          requestId={failure.requestId}
          onRetry={
            !failure.final && preview.kind === "loading" && !waiting
              ? () => {
                  setFailure(undefined);
                  setAttempt(attempt + 1);
                }
              : undefined
          }
          takeFocus
        >
          {failure.final && <ToSignInLink />}
        </InlineError>
      )}
    </PublicPage>
  );
}
