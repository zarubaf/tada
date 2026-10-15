import { useEffect } from "react";
import { useNavigate } from "../router/Router";
import { type ContactsTab, contactsAddress } from "./contactsTab";

/**
 * The old address of a register, `/persons` or `/institutions`, leads to its tab of „Kontakte“
 * (ADR 0082). It replaces the history entry and keeps the query and the fragment, so that a link
 * to a row (`#PER-003`) still finds the row. Focus stays: no member caused the redirect.
 */
export function ContactsRedirect({ tab }: { tab: ContactsTab }) {
  const navigate = useNavigate();
  useEffect(() => {
    const { search, hash } = window.location;
    navigate(`${contactsAddress(tab, new URLSearchParams(search))}${hash}`, {
      replace: true,
      moveFocus: false,
    });
  }, [navigate, tab]);
  return null;
}
