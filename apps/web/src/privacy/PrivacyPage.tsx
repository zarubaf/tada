import { useCallback, useEffect, useRef, useState } from "react";
import { type Api, problemMessage } from "../api/client";
import { t } from "../i18n";
import { useRetry } from "../ui/focus";
import { InlineError } from "../ui/InlineError";
import { Page, PageTitle } from "../ui/Page";
import { Skeleton } from "../ui/Skeleton";
import { PrivacyNoticeText } from "./PrivacyNoticeText";

type State =
  | { kind: "loading" }
  | { kind: "failed"; message: string; requestId: string | undefined }
  | { kind: "loaded"; markdown: string | null };

/** „Datenschutz“: the privacy notice of the organization, for each member. */
export function PrivacyPage({ api }: { api: Api }) {
  const [state, setState] = useState<State>({ kind: "loading" });
  const heading = useRef<HTMLHeadingElement>(null);
  const { retried, retry } = useRetry(() => heading.current);

  /** Resolves to true when the notice loaded. */
  const load = useCallback(async () => {
    try {
      const { data, error } = await api.GET("/api/v1/organization/privacy-notice");
      if (data) {
        setState({ kind: "loaded", markdown: data.markdown });
        return true;
      }
      setState({ kind: "failed", message: problemMessage(error), requestId: error?.request_id });
    } catch {
      setState({ kind: "failed", message: problemMessage(undefined), requestId: undefined });
    }
    return false;
  }, [api]);

  useEffect(() => {
    void load();
  }, [load]);

  return (
    <Page>
      <PageTitle ref={heading}>{t("privacy-title")}</PageTitle>
      {state.kind === "loading" && (
        <div role="status" aria-label={t("privacy-loading")}>
          <Skeleton />
          <Skeleton />
          <Skeleton />
        </div>
      )}
      {state.kind === "failed" && (
        <InlineError
          message={state.message}
          requestId={state.requestId}
          onRetry={() =>
            retry(() => {
              setState({ kind: "loading" });
              return load();
            })
          }
          announce={retried ? "focus" : "alert"}
        />
      )}
      {state.kind === "loaded" && <PrivacyNoticeText markdown={state.markdown} />}
    </Page>
  );
}
