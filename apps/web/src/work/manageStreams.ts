// Whether the „Bereiche verwalten“ dialog is open over the „Arbeit“ page (ADR 0082). The query key
// `manage=streams` names it, so that the command palette can open it by an address and Back closes
// it. The other query keys, the view and the filters, stay.
import { useCallback } from "react";
import { useNavigate, useSearch } from "../router/Router";

const KEY = "manage";
const VALUE = "streams";

/** The address of the „Arbeit“ page with the dialog open, for the command palette. */
export const manageStreamsAddress = (eventId: string): string =>
  `/events/${encodeURIComponent(eventId)}/work?${KEY}=${VALUE}`;

export function useManageStreams() {
  const search = useSearch();
  const navigate = useNavigate();
  const isOpen = search.get(KEY) === VALUE;

  const go = useCallback(
    (open: boolean) => {
      const params = new URLSearchParams(window.location.search);
      params.delete(KEY);
      if (open) {
        params.set(KEY, VALUE);
      }
      const query = params.toString();
      navigate(`${window.location.pathname}${query ? `?${query}` : ""}${window.location.hash}`, {
        replace: !open,
        moveFocus: false,
      });
    },
    [navigate],
  );

  return {
    isOpen,
    /** Opens the dialog. It makes a history entry, so that Back closes it. */
    open: useCallback(() => go(true), [go]),
    /** Closes the dialog and keeps the query. It replaces the history entry. */
    close: useCallback(() => go(false), [go]),
  };
}
