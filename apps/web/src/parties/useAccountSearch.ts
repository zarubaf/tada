// The search for the account to link to a person: members that match a text. The server searches
// by name, and by email address for an owner or an admin; a few results are enough to pick from.
import { useEffect, useState } from "react";
import { type Api, problemMessage } from "../api/client";

const DEBOUNCE_MS = 250;
const RESULTS = 6;

export interface AccountSearch {
  options: { id: string; label: string }[];
  loading: boolean;
  /** The message of a failed search. */
  failure: string | undefined;
}

/** Searches after a short pause in the typing. `text` empty lists the first members. */
export function useAccountSearch(
  api: Api | undefined,
  text: string,
  enabled: boolean,
): AccountSearch {
  const [state, setState] = useState<AccountSearch>({
    options: [],
    loading: true,
    failure: undefined,
  });
  useEffect(() => {
    if (!enabled || api === undefined) {
      return;
    }
    let current = true;
    setState((previous) => ({ ...previous, loading: true }));
    const timer = setTimeout(async () => {
      let next: AccountSearch;
      try {
        const query = { limit: RESULTS, ...(text.trim() === "" ? {} : { q: text.trim() }) };
        const { data, error } = await api.GET("/api/v1/members", { params: { query } });
        next = data
          ? {
              loading: false,
              failure: undefined,
              options: data.items.map((m) => ({
                id: m.user_id,
                label: m.email ? `${m.display_name} (${m.email})` : m.display_name,
              })),
            }
          : { options: [], loading: false, failure: problemMessage(error) };
      } catch {
        next = { options: [], loading: false, failure: problemMessage(undefined) };
      }
      if (current) {
        setState(next);
      }
    }, DEBOUNCE_MS);
    return () => {
      current = false;
      clearTimeout(timer);
    };
  }, [api, text, enabled]);
  return state;
}
