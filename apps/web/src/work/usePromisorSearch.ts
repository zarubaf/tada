// The search for the promisor of a commitment: persons and institutions that match a text. The
// server searches by name (ADR 0069); a few results are enough to pick from.
import { useEffect, useState } from "react";
import { type Api, problemMessage } from "../api/client";
import type { Choice } from "./directory";

const DEBOUNCE_MS = 250;
const RESULTS = 6;

export interface PromisorSearch {
  /** The ID of an option is `person:{uuid}` or `institution:{uuid}`. */
  options: Choice[];
  loading: boolean;
  /** The message of a failed search. */
  failure: string | undefined;
}

/** Searches after a short pause in the typing. `text` empty lists the first records. */
export function usePromisorSearch(api: Api, text: string, enabled: boolean): PromisorSearch {
  const [state, setState] = useState<PromisorSearch>({
    options: [],
    loading: true,
    failure: undefined,
  });
  useEffect(() => {
    if (!enabled) {
      return;
    }
    let current = true;
    setState((previous) => ({ ...previous, loading: true }));
    const timer = setTimeout(async () => {
      let next: PromisorSearch;
      try {
        const query = { limit: RESULTS, ...(text.trim() === "" ? {} : { q: text.trim() }) };
        const [persons, institutions] = await Promise.all([
          api.GET("/api/v1/persons", { params: { query } }),
          api.GET("/api/v1/institutions", { params: { query } }),
        ]);
        next =
          persons.data && institutions.data
            ? {
                loading: false,
                failure: undefined,
                options: [
                  ...persons.data.items.map((p) => ({
                    id: `person:${p.id}`,
                    label: `${p.name} (${p.local_id})`,
                  })),
                  ...institutions.data.items.map((i) => ({
                    id: `institution:${i.id}`,
                    label: `${i.name} (${i.local_id})`,
                  })),
                ],
              }
            : {
                options: [],
                loading: false,
                failure: problemMessage(persons.error ?? institutions.error),
              };
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
