import { useCallback, useMemo } from "react";
import { useNavigate, useSearch } from "../router/Router";
import type { PartyKind } from "./partyApi";

/** The tabs of „Kontakte“ (ADR 0082). The persons are the default. */
export const CONTACT_TABS = ["persons", "institutions"] as const;
export type ContactsTab = (typeof CONTACT_TABS)[number];

const TAB = "tab";
export const DEFAULT_TAB: ContactsTab = "persons";

/** The kind of record that a tab lists. */
export const KIND_OF_TAB: Record<ContactsTab, PartyKind> = {
  persons: "person",
  institutions: "institution",
};

/** Reads the tab from a query string. What it does not know, it shows as the persons. */
export function parseContactsTab(params: Pick<URLSearchParams, "get">): ContactsTab {
  const tab = params.get(TAB);
  return CONTACT_TABS.find((candidate) => candidate === tab) ?? DEFAULT_TAB;
}

/**
 * The tab of the page, kept in the query key `tab`; the persons have no key. A change makes a new
 * history entry, because a tab is a place to go back to. It drops the fragment: the readable ID of
 * a row belongs to the register that the fragment named.
 */
export function useContactsTab(): [ContactsTab, (next: ContactsTab) => void] {
  const search = useSearch();
  const navigate = useNavigate();
  const tab = useMemo(() => parseContactsTab(search), [search]);
  const set = useCallback(
    (next: ContactsTab) =>
      navigate(contactsAddress(next, new URLSearchParams(window.location.search)), {
        moveFocus: false,
      }),
    [navigate],
  );
  return [tab, set];
}

/** The address of the page with the tab, without a key for the default. */
export function contactsAddress(tab: ContactsTab, rest = new URLSearchParams()): string {
  const params = new URLSearchParams(rest);
  params.delete(TAB);
  if (tab !== DEFAULT_TAB) {
    params.set(TAB, tab);
  }
  const query = params.toString();
  return `/contacts${query ? `?${query}` : ""}`;
}
