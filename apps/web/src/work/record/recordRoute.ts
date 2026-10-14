// Which record the „Arbeit“ page shows in its side panel, and whether the panel creates one
// (ADR 0082). The hash names the open record by its readable ID, for example `#ACT-012`. The query
// key `new` names the kind to create, for example `?new=action`. Both keep the other query keys,
// so the view and the filters stay.
import { useCallback, useMemo } from "react";
import type { RecordKind } from "../../api/client";
import { useHash, useNavigate, useSearch } from "../../router/Router";

/** The query key that opens the create state of the panel. */
export const NEW_PARAM = "new";

/** The kinds that the panel can create. A kind joins with its descriptor. */
export const CREATABLE: readonly RecordKind[] = ["action", "risk", "milestone"];

export type RecordRoute =
  | { mode: "closed" }
  | { mode: "open"; id: string }
  | { mode: "create"; kind: RecordKind };

export interface RecordRouteControls {
  route: RecordRoute;
  /**
   * Shows a record, also the one that the create state made, and drops the key to create. It
   * replaces the history entry: prev and next are no places to go back to.
   */
  open: (readableId: string) => void;
  /** Closes the panel and keeps the query. It replaces the history entry. */
  close: () => void;
  /** Opens the create state. It makes a history entry, so that back closes it. */
  startCreate: (kind: RecordKind) => void;
}

function routeOf(hash: string, creating: string | null): RecordRoute {
  const kind = CREATABLE.find((candidate) => candidate === creating);
  if (kind) {
    return { mode: "create", kind };
  }
  // The readable IDs are upper case; an address typed by hand may not be.
  return hash === "" ? { mode: "closed" } : { mode: "open", id: hash.toUpperCase() };
}

export function useRecordRoute(): RecordRouteControls {
  const hash = useHash();
  const search = useSearch();
  const navigate = useNavigate();
  const creating = search.get(NEW_PARAM);
  const route = useMemo(() => routeOf(hash, creating), [hash, creating]);

  const go = useCallback(
    (to: { hash?: string; kind?: RecordKind }, replace: boolean) => {
      const params = new URLSearchParams(window.location.search);
      params.delete(NEW_PARAM);
      if (to.kind) {
        params.set(NEW_PARAM, to.kind);
      }
      const query = params.toString();
      const fragment = to.hash === undefined ? "" : `#${encodeURIComponent(to.hash)}`;
      navigate(`${window.location.pathname}${query ? `?${query}` : ""}${fragment}`, {
        replace,
        moveFocus: false,
      });
    },
    [navigate],
  );

  return {
    route,
    open: useCallback((id) => go({ hash: id }, true), [go]),
    close: useCallback(() => go({}, true), [go]),
    startCreate: useCallback((kind) => go({ kind }, false), [go]),
  };
}
