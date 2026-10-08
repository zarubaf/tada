import { useCallback, useEffect, useRef, useState } from "react";
import type { Api, InvitationPreview } from "../api/client";
import { type Failure, failureOf, invalidFailure, useWaiting } from "../api/failure";
import { t } from "../i18n";
import { useNavigate } from "../router/Router";
import { useRefreshSession } from "../session/SessionProvider";
import { Button } from "../ui/Button";
import { useRetry } from "../ui/focus";
import { InlineError } from "../ui/InlineError";
import { LiveRegion } from "../ui/LiveRegion";
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
  const [busy, setBusy] = useState(false);
  const waiting = useWaiting(failure);
  const refresh = useRefreshSession();
  const navigate = useNavigate();

  const heading = useRef<HTMLHeadingElement>(null);
  const { retried, retry } = useRetry(() => heading.current);

  // Only the newest preview request may change the page; an older answer arrives late and is stale.
  const latestPreview = useRef(0);

  /** The preview uses no token up. Resolves to true when it loaded. */
  const loadPreview = useCallback(async () => {
    if (!token) {
      return false;
    }
    const request = ++latestPreview.current;
    const isStale = () => request !== latestPreview.current;
    try {
      const result = await api.POST("/api/v1/invitations/preview", { body: { token } });
      if (isStale()) {
        return false;
      }
      if (result.data) {
        setFailure(undefined);
        setPreview({ kind: "ready", preview: result.data });
        return true;
      }
      setFailure(failureOf(result, t("invitation-invalid")));
    } catch {
      if (!isStale()) {
        setFailure(failureOf({}));
      }
    }
    return false;
  }, [api, token]);

  useEffect(() => {
    void loadPreview();
  }, [loadPreview]);

  const accept = async () => {
    if (!token || busy || waiting) {
      return;
    }
    // Clear the message first, so that an identical one is announced again.
    setFailure(undefined);
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

  // The accept button is in the page: its failure leaves it there.
  const acceptStays = preview.kind === "ready" && failure !== undefined && !failure.final;

  return (
    <PublicPage title={t("invitation-title")} titleRef={heading}>
      <LiveRegion kind="status">
        {preview.kind === "loading" && !failure ? t("invitation-loading") : ""}
      </LiveRegion>
      {preview.kind === "ready" && !failure?.final && (
        <>
          <PublicText>
            {t("invitation-text", {
              organization: preview.preview.organization_name,
              role: t(`role-${preview.preview.role}`),
            })}
          </PublicText>
          <Button variant="primary" isPending={busy || waiting} onPress={() => void accept()}>
            {t("invitation-accept")}
          </Button>
        </>
      )}
      {/* After a failed accept that is not final, the button stays and keeps focus. */}
      <LiveRegion kind="alert" visuallyHidden>
        {acceptStays ? failure?.message : undefined}
      </LiveRegion>
      {failure && (
        <InlineError
          key={failure.id}
          message={failure.message}
          requestId={failure.requestId}
          onRetry={
            !failure.final && preview.kind === "loading" && !waiting
              ? () =>
                  retry(() => {
                    setFailure(undefined);
                    return loadPreview();
                  })
              : undefined
          }
          announce={acceptStays ? "none" : failure.final || retried ? "focus" : "alert"}
        >
          {failure.final && <ToSignInLink />}
        </InlineError>
      )}
    </PublicPage>
  );
}
