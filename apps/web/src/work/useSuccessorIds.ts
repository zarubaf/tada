// The readable ID of the decision that replaced another. The successor has a higher number and is
// often on a page that the register has not loaded, so the hook asks the server for those that
// the loaded rows lack.
import { useEffect, useRef, useState } from "react";
import type { Api, Decision } from "../api/client";

/** Returns the readable ID of a decision by its UUID, once the page or the server has told it. */
export function useSuccessorIds(
  api: Api,
  eventId: string,
  rows: Decision[],
): (id: string) => string | undefined {
  const [fetched, setFetched] = useState<Record<string, string>>({});
  const asked = useRef(new Set<string>());
  const loaded = new Set(rows.map((row) => row.id));
  const missing = [
    ...new Set(rows.flatMap((row) => (row.superseded_by ? [row.superseded_by] : []))),
  ]
    .filter((id) => !loaded.has(id))
    .sort()
    .join(" ");

  useEffect(() => {
    for (const id of missing === "" ? [] : missing.split(" ")) {
      if (asked.current.has(id)) {
        continue;
      }
      asked.current.add(id);
      api
        .GET("/api/v1/events/{event_id}/decisions/{id}", {
          params: { path: { event_id: eventId, id } },
        })
        .then(({ data }) => {
          if (data) {
            setFetched((previous) => ({ ...previous, [id]: data.local_id }));
          } else {
            asked.current.delete(id);
          }
        })
        .catch(() => asked.current.delete(id));
    }
  }, [api, eventId, missing]);

  return (id) => rows.find((row) => row.id === id)?.local_id ?? fetched[id];
}
