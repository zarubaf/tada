// Which record the „Arbeit“ page shows in its side panel, and whether the panel creates one
// (ADR 0082). The hash names the open record by its readable ID, for example `#ACT-012`. The query
// key `new` names the kind to create, for example `?new=action`. Both keep the other query keys,
// so the view and the filters stay.
import { useCallback, useMemo } from "react";
import type { RecordKind } from "../../api/client";
import { useHash, useNavigate, useSearch } from "../../router/Router";

/** The query key that opens the create state of the panel. */
export const NEW_PARAM = "new";

/** The kinds that the panel can create, in the order of the „Neu“ menu. A kind joins with its descriptor. */
export const CREATABLE: readonly RecordKind[] = [
  "action",
  "commitment",
  "milestone",
  "decision",
  "risk",
];

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
  /**
   * Shows a record that the member chose by a link of the panel. It makes a history entry, so
   * that Back returns to the record they came from.
   */
  follow: (readableId: string) => void;
  /** Closes the panel and keeps the query. It replaces the history entry. */
  close: () => void;
  /** Opens the create state. It makes a history entry, so that back closes it. */
  startCreate: (kind: RecordKind) => void;
}

/** The shape of a readable ID: three letters for the kind, a number, for example `ACT-012`. */
const READABLE_ID = /^[A-Z]{3}-\d+$/i;

function routeOf(hash: string, creating: string | null): RecordRoute {
  const kind = CREATABLE.find((candidate) => candidate === creating);
  if (kind) {
    return { mode: "create", kind };
  }
  // Any other hash, such as the anchor of the skip link, names no record.
  // The readable IDs are upper case; an address typed by hand may not be.
  return READABLE_ID.test(hash) ? { mode: "open", id: hash.toUpperCase() } : { mode: "closed" };
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
    follow: useCallback((id) => go({ hash: id }, false), [go]),
    close: useCallback(() => go({}, true), [go]),
    startCreate: useCallback((kind) => go({ kind }, false), [go]),
  };
}
