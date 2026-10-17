// The switch rule of ADR 0084, once: the event switcher and the command menu both use it.
import { useNavigate, usePathname, useSearch } from "../router/Router";
import { eventIdOf } from "./navItems";

/** The pages of an Anlass that a switch keeps. Everything else of an Anlass opens Übersicht. */
const KEPT_PAGES = ["work", "documents"];

/**
 * The address to open when the member switches to the Anlass `targetId`. On a page of an Anlass,
 * „Arbeit“ and „Dokumente“ stay (a document goes to the documents), with the `view` parameter of
 * the work page; every other page opens Übersicht of the new Anlass. Filters and the hash drop.
 */
export function eventAddress(pathname: string, view: string | null, targetId: string): string {
  const base = `/events/${encodeURIComponent(targetId)}`;
  const current = eventIdOf(pathname);
  const page = current === undefined ? undefined : pathname.split("/")[3];
  if (current === targetId) {
    return view === null ? pathname : `${pathname}?view=${encodeURIComponent(view)}`;
  }
  if (page === "work") {
    return view === null ? `${base}/work` : `${base}/work?view=${encodeURIComponent(view)}`;
  }
  if (page !== undefined && KEPT_PAGES.includes(page)) {
    return `${base}/${page}`;
  }
  return base;
}

/** Switches to the Anlass with the ID: it keeps the page where the rule says so. */
export function useSwitchEvent(): (targetId: string) => void {
  const navigate = useNavigate();
  const pathname = usePathname();
  const view = useSearch().get("view");
  return (targetId) => navigate(eventAddress(pathname, view, targetId));
}
