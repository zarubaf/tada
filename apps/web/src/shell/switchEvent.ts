// The switch rule of ADR 0084, once: the event switcher and the command menu both use it.
import { useNavigate, usePathname, useSearch } from "../router/Router";
import { DEFAULT_WORK_VIEW, parseWorkView } from "../work/workView";
import { eventIdOf } from "./navItems";

/** The pages of an event that a switch keeps. Everything else of an event opens Übersicht. */
const KEPT_PAGES = ["documents"];

/**
 * The address to open when the member switches to the event `targetId`. On a page of an event,
 * „Arbeit“ and „Dokumente“ stay (a document goes to the documents), with the `view` parameter of
 * the work page; every other page opens Übersicht of the new event. Filters and the hash drop.
 * Choosing the event that is open is not a switch: `useSwitchEvent` leaves the address as it is.
 */
export function eventAddress(pathname: string, view: string | null, targetId: string): string {
  const base = `/events/${encodeURIComponent(targetId)}`;
  const page = eventIdOf(pathname) === undefined ? undefined : pathname.split("/")[3];
  if (page === "work") {
    // Only a known view that is not the default stays; the default view has no key.
    const kept = parseWorkView({ get: () => view });
    return kept === DEFAULT_WORK_VIEW ? `${base}/work` : `${base}/work?view=${kept}`;
  }
  if (page !== undefined && KEPT_PAGES.includes(page)) {
    return `${base}/${page}`;
  }
  return base;
}

/** Switches to the event with the ID: it keeps the page where the rule says so. */
export function useSwitchEvent(): (targetId: string) => void {
  const navigate = useNavigate();
  const pathname = usePathname();
  const view = useSearch().get("view");
  return (targetId) => {
    if (eventIdOf(pathname) !== targetId) {
      navigate(eventAddress(pathname, view, targetId));
    }
  };
}
