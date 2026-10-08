// The list of open changesets, loaded once for the shell and the Review Inbox. The count in the
// navigation and the list of the inbox come from the same request (ADR 0050).

import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from "react";
import { type Api, type OpenChangeset, problemMessage } from "../api/client";
import { useOptionalSession } from "../session/SessionProvider";

export type InboxState =
  | { kind: "loading" }
  | { kind: "failed"; message: string; requestId: string | undefined }
  /** `more` is true if the server has more open changesets than the page holds. */
  | { kind: "loaded"; items: OpenChangeset[]; more: boolean };

export interface Inbox {
  state: InboxState;
  /** Loads the list again. The loaded list stays until the new one arrives. Resolves to true on success. */
  reload: () => Promise<boolean>;
}

const InboxContext = createContext<Inbox | undefined>(undefined);

/** The page size is the largest the API allows (ADR 0044). */
const PAGE_SIZE = 200;

/**
 * Loads the open changesets of the organization of the session. It loads nothing without an
 * organization, and loads again when the member switches the organization.
 */
export function InboxProvider({ api, children }: { api: Api; children: ReactNode }) {
  const organizationId = useOptionalSession()?.organization?.organization_id;
  const [state, setState] = useState<InboxState>({ kind: "loading" });

  const reload = useCallback(async () => {
    try {
      const { data, error } = await api.GET("/api/v1/changesets", {
        params: { query: { status: "open", limit: PAGE_SIZE } },
      });
      if (data) {
        setState({
          kind: "loaded",
          items: data.items,
          more: data.next_cursor != null,
        });
        return true;
      }
      setState({ kind: "failed", message: problemMessage(error), requestId: error?.request_id });
    } catch {
      setState({ kind: "failed", message: problemMessage(undefined), requestId: undefined });
    }
    return false;
  }, [api]);

  useEffect(() => {
    if (organizationId === undefined) {
      return;
    }
    setState({ kind: "loading" });
    void reload();
  }, [organizationId, reload]);

  const value = useMemo(() => ({ state, reload }), [state, reload]);
  return <InboxContext value={value}>{children}</InboxContext>;
}

/** The inbox, or nothing where no provider exists, for example in the shell tests. */
export function useOptionalInbox(): Inbox | undefined {
  return useContext(InboxContext);
}

/** The inbox. Only a page inside `InboxProvider` can call it. */
export function useInbox(): Inbox {
  const inbox = useContext(InboxContext);
  if (!inbox) {
    throw new Error("useInbox needs an InboxProvider around it");
  }
  return inbox;
}
