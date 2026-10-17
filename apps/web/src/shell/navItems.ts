// The destinations of the shell, once: the sidebar, the bottom bar, the menu „Mehr“ and the command
// palette all read this list. An item appears only when its page exists (ADR 0082).
import {
  type Icon,
  IconCalendarEvent,
  IconFiles,
  IconInbox,
  IconLayoutDashboard,
  IconLayoutKanban,
  IconSettings,
  IconShieldLock,
  IconTimeline,
  IconUser,
  IconUsers,
} from "@tabler/icons-react";
import { t } from "../i18n";
import { passesQueryRules, type QueryRule } from "../router/Router";

export interface Destination {
  id: string;
  to: string;
  label: string;
  icon: Icon;
  /** Current only for its own path. */
  exact?: boolean;
  /** Current also below this prefix, when the path of the link is not a prefix of its pages. */
  within?: string;
  /** Current only while the query has this value: „Zeitplan“ is a view of the work page. */
  onlyWith?: QueryRule;
  /** Not current while the query has this value: the sibling of `onlyWith`. */
  unlessWith?: QueryRule;
}

/** The view of the work page that the „Zeitplan“ item opens. */
const TIMELINE: QueryRule = { name: "view", value: "timeline" };

const inbox: Destination = { id: "inbox", to: "/inbox", label: t("nav-inbox"), icon: IconInbox };
const myWork: Destination = {
  id: "my-work",
  to: "/",
  label: t("nav-my-work"),
  icon: IconUser,
  exact: true,
};
/** The list of all events: not a menu item (ADR 0084), but a destination of the palette. */
const events: Destination = {
  id: "events",
  to: "/events",
  label: t("events-title"),
  icon: IconCalendarEvent,
  exact: true,
};
/** „Kontakte“: the persons and the institutions are its two tabs (ADR 0082). */
const contacts: Destination = {
  id: "contacts",
  to: "/contacts",
  label: t("nav-contacts"),
  icon: IconUsers,
};
const settings: Destination = {
  id: "settings",
  to: "/settings",
  label: t("nav-settings"),
  icon: IconSettings,
};
const privacy: Destination = {
  id: "privacy",
  to: "/privacy",
  label: t("nav-privacy"),
  icon: IconShieldLock,
};

/** What the event items look like, with no event: the disabled menu has the same four items. */
const face = (id: string, key: string, icon: Icon) => ({ id, label: t(key), icon });
const OVERVIEW = face("overview", "nav-overview", IconLayoutDashboard);
const WORK = face("work", "nav-work", IconLayoutKanban);
const TIMELINE_ITEM = face("timeline", "nav-timeline", IconTimeline);
const DOCUMENTS = face("documents", "nav-documents", IconFiles);
export const EVENT_ITEM_FACES = [OVERVIEW, WORK, TIMELINE_ITEM, DOCUMENTS];

/** „Übersicht“ of one event: its home, current only on its own path (ADR 0084). */
function overview(eventId: string): Destination {
  return {
    ...OVERVIEW,
    to: `/events/${encodeURIComponent(eventId)}`,
    exact: true,
  };
}

/** „Arbeit“ of one event: all its work records on one page (ADR 0082). */
function work(eventId: string): Destination {
  return {
    ...WORK,
    to: `/events/${encodeURIComponent(eventId)}/work`,
    unlessWith: TIMELINE,
  };
}

/** „Zeitplan“ of one event: the timeline view of the work page (ADR 0082). */
function timeline(eventId: string): Destination {
  return {
    ...TIMELINE_ITEM,
    to: `/events/${encodeURIComponent(eventId)}/work?view=${TIMELINE.value}`,
    onlyWith: TIMELINE,
  };
}

/** „Dokumente“ of one event: its documents and their versions. */
function documents(eventId: string): Destination {
  return {
    ...DOCUMENTS,
    to: `/events/${encodeURIComponent(eventId)}/documents`,
  };
}

/** The items of the member's organization; they do not depend on the event. */
export const ORGANIZATION_ITEMS: Destination[] = [inbox, myWork, contacts];
/** The items of the scoped event, below the event switcher (ADR 0084). */
export function eventItems(eventId: string): Destination[] {
  return [overview(eventId), work(eventId), timeline(eventId), documents(eventId)];
}
export const SETTINGS = settings;
/** A place of the bottom bar or of „Mehr“: without a target it is the inert face of an Anlass item. */
export type BarItem = Destination | (Omit<Destination, "to"> & { to?: undefined });
/** True when the place has a target. */
export function hasTarget(item: BarItem): item is Destination {
  return item.to !== undefined;
}
/**
 * The bottom bar on narrow layouts: the same four places in every state (ADR 0084). „Arbeit“ is
 * the one of the scoped Anlass; with no Anlass it has no target.
 */
export function bottomItems(eventId: string | undefined): BarItem[] {
  return [inbox, myWork, eventId === undefined ? WORK : work(eventId)];
}
/** „Mehr“ holds the other destinations, the same list in every state; the Anlass items need an Anlass. */
export function moreItems(eventId: string | undefined): BarItem[] {
  return [
    eventId === undefined ? OVERVIEW : overview(eventId),
    eventId === undefined ? DOCUMENTS : documents(eventId),
    contacts,
    settings,
    privacy,
  ];
}
export const PRIVACY = privacy;
/** Each destination, for the command palette. */
export const ALL: Destination[] = [...ORGANIZATION_ITEMS, events, settings, privacy];

/** True when the path belongs to the destination. */
export function isCurrent(destination: Destination, pathname: string, search = ""): boolean {
  if (!passesQueryRules(search, destination)) {
    return false;
  }
  // A query in `to` names a view; the path decides which page the item belongs to.
  const target = destination.to.split(/[?#]/, 1)[0] ?? destination.to;
  return (
    pathname === target ||
    (!destination.exact && pathname.startsWith(`${target}/`)) ||
    (destination.within !== undefined && pathname.startsWith(`${destination.within}/`))
  );
}

/**
 * The label of the destination that the path belongs to, or nothing for an unknown path. The pages
 * below `/events` have no destination: „Alle Anlässe“ names the list only, so the title never
 * contradicts the event switcher.
 */
export function sectionTitle(pathname: string): string | undefined {
  if (pathname === "/events/new") {
    return t("event-create-title");
  }
  return ALL.find((destination) => isCurrent(destination, pathname))?.label;
}

/** The ID of the event that the path names, for example `/events/<id>/work`; not `/events/new`. */
export function eventIdOf(pathname: string): string | undefined {
  const id = /^\/events\/([^/]+)/.exec(pathname)?.[1];
  if (id === undefined || id === "new") {
    return undefined;
  }
  try {
    return decodeURIComponent(id);
  } catch {
    return undefined;
  }
}
