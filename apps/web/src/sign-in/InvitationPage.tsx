import { useEffect, useState } from "react";
import { type Api, type InvitationPreview, type Problem, problemMessage } from "../api/client";
import { t } from "../i18n";
import { useNavigate } from "../router/Router";
import { useRefreshSession } from "../session/SessionProvider";
import { Button } from "../ui/Button";
import { InlineError } from "../ui/InlineError";
import { Skeleton } from "../ui/Skeleton";
import { takeFragmentToken } from "./fragment";
import { PublicPage, PublicText, ToSignInLink } from "./PublicPage";

type State =
  | { kind: "loading" }
  | { kind: "ready"; preview: InvitationPreview }
  | { kind: "failed"; message: string; requestId: string | undefined };

const INVALID: State = {
  kind: "failed",
  message: t("invitation-invalid"),
  requestId: undefined,
};

function failed(error: Problem | undefined): State {
  // A 4xx means that the invitation does not work. Only the failures of the server are worth a note.
  if (error && error.status < 500) {
    return INVALID;
  }
  return { kind: "failed", message: problemMessage(error), requestId: error?.request_id };
}

/**
 * The page that an invitation link opens. The preview shows the organization and the role without
 * using the token. Only the click accepts the invitation (ADR 0008, ADR 0056).
 */
export function InvitationPage({ api }: { api: Api }) {
  // The first render reads and removes the fragment. A later render must keep the token.
  const [token] = useState(takeFragmentToken);
  const [state, setState] = useState<State>(token ? { kind: "loading" } : INVALID);
  const [busy, setBusy] = useState(false);
  const refresh = useRefreshSession();
  const navigate = useNavigate();

  useEffect(() => {
    if (!token) {
      return;
    }
    let current = true;
    api
      .POST("/api/v1/invitations/preview", { body: { token } })
      .then(({ data, error }) => {
        if (current) {
          setState(data ? { kind: "ready", preview: data } : failed(error));
        }
      })
      .catch(() => current && setState(failed(undefined)));
    return () => {
      current = false;
    };
  }, [api, token]);

  const accept = async () => {
    if (!token) {
      return;
    }
    setBusy(true);
    try {
      const { error } = await api.POST("/api/v1/invitations/accept", { body: { token } });
      if (!error) {
        await refresh();
        navigate("/events", { replace: true });
        return;
      }
      setState(failed(error));
    } catch {
      setState(failed(undefined));
    }
    setBusy(false);
  };

  return (
    <PublicPage title={t("invitation-title")}>
      {state.kind === "loading" && (
        <div role="status" aria-label={t("invitation-loading")}>
          <Skeleton />
        </div>
      )}
      {state.kind === "failed" && (
        <InlineError message={state.message} requestId={state.requestId}>
          <ToSignInLink />
        </InlineError>
      )}
      {state.kind === "ready" && (
        <>
          <PublicText>
            {t("invitation-text", {
              organization: state.preview.organization_name,
              role: t(`role-${state.preview.role}`),
            })}
          </PublicText>
          <Button variant="primary" isDisabled={busy} onPress={() => void accept()}>
            {t("invitation-accept")}
          </Button>
        </>
      )}
    </PublicPage>
  );
}
