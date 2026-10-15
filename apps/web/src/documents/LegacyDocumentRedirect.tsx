import { useCallback, useEffect, useState } from "react";
import { type Api, problemMessage } from "../api/client";
import { t } from "../i18n";
import { Redirect, useParams, useSearch } from "../router/Router";
import { InlineError } from "../ui/InlineError";
import { Page } from "../ui/Page";
import { Skeleton } from "../ui/Skeleton";
import { documentPath } from "./versions";

type State =
  | { kind: "loading" }
  | { kind: "failed"; message: string; requestId: string | undefined }
  | { kind: "found"; eventId: string };

/**
 * The old address of a document (`/documents/:documentId`, with `/diff`) has no event. Links in
 * mail, digests and bookmarks still use it, so it looks up the event of the document and leads to
 * the address below the event (the layout of the event needs it in the path).
 */
export function LegacyDocumentRedirect({ api, diff = false }: { api: Api; diff?: boolean }) {
  const { documentId = "" } = useParams();
  const search = useSearch();
  const [state, setState] = useState<State>({ kind: "loading" });

  const load = useCallback(async () => {
    try {
      const { data, error } = await api.GET("/api/v1/documents/{document_id}", {
        params: { path: { document_id: documentId } },
      });
      setState(
        data
          ? { kind: "found", eventId: data.event_id }
          : { kind: "failed", message: problemMessage(error), requestId: error?.request_id },
      );
    } catch {
      setState({ kind: "failed", message: problemMessage(undefined), requestId: undefined });
    }
  }, [api, documentId]);

  useEffect(() => {
    void load();
  }, [load]);

  if (state.kind === "found") {
    const query = search.toString();
    const suffix = diff ? `/diff${query ? `?${query}` : ""}` : "";
    return <Redirect to={`${documentPath(state.eventId, documentId)}${suffix}`} />;
  }
  return (
    <Page width="flush">
      {state.kind === "failed" ? (
        <InlineError
          message={state.message}
          requestId={state.requestId}
          onRetry={() => {
            setState({ kind: "loading" });
            void load();
          }}
        />
      ) : (
        <div role="status" aria-label={t("document-loading")}>
          <Skeleton />
        </div>
      )}
    </Page>
  );
}
