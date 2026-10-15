// The destinations of the shell, once: the sidebar, the bottom bar, the menu „Mehr“ and the command
// palette all read this list. An item appears only when its page exists (ADR 0082).
import {
  type Icon,
  IconBuilding,
  IconCalendarEvent,
  IconFiles,
  IconInbox,
  IconLayoutKanban,
  IconSettings,
  IconShieldLock,
  IconTimeline,
  IconUser,
  IconUsers,
} from "@tabler/icons-react";
import { t } from "../i18n";
import { passesQueryRules, type QueryRule, yieldsAt } from "../router/Router";

export interface Destination {
  id: string;
  to: string;
  label: string;
  icon: Icon;
  /** Current only for its own path. */
  exact?: boolean;
  /** Current also below this prefix, when the path of the link is not a prefix of its pages. */
  within?: string;
  /** Another item claims this path and the paths below it, so this item is not current there. */
  yieldsTo?: string | readonly string[];
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
const events: Destination = {
  id: "events",
  to: "/events",
  label: t("nav-events"),
  icon: IconCalendarEvent,
};
const persons: Destination = {
  id: "persons",
  to: "/persons",
  label: t("nav-persons"),
  icon: IconUsers,
};
const institutions: Destination = {
  id: "institutions",
  to: "/institutions",
  label: t("nav-institutions"),
  icon: IconBuilding,
};
const settings: Destination = {
  id: "settings",
  to: "/settings/members",
  label: t("nav-settings"),
  icon: IconSettings,
  within: "/settings",
};
const privacy: Destination = {
  id: "privacy",
  to: "/privacy",
  label: t("nav-privacy"),
  icon: IconShieldLock,
};

/** „Arbeit“ of one event: all its work records on one page (ADR 0082). */
function work(eventId: string): Destination {
  return {
    id: "work",
    to: `/events/${encodeURIComponent(eventId)}/work`,
    label: t("nav-work"),
    icon: IconLayoutKanban,
    unlessWith: TIMELINE,
  };
}

/** „Zeitplan“ of one event: the timeline view of the work page (ADR 0082). */
function timeline(eventId: string): Destination {
  return {
    id: "timeline",
    to: `/events/${encodeURIComponent(eventId)}/work?view=${TIMELINE.value}`,
    label: t("nav-timeline"),
    icon: IconTimeline,
    onlyWith: TIMELINE,
  };
}

/** „Dokumente“ of one event: its documents and their versions. */
function documents(eventId: string): Destination {
  return {
    id: "documents",
    to: `/events/${encodeURIComponent(eventId)}/documents`,
    label: t("nav-documents"),
    icon: IconFiles,
  };
}

/** „Anlässe“. Inside an event it yields the pages that have an item of their own. */
function eventsItem(eventId: string | undefined): Destination {
  return eventId === undefined
    ? events
    : { ...events, yieldsTo: [work(eventId).to, documents(eventId).to] };
}

/**
 * The items of the sidebar and of the drawer, from top to bottom. Einstellungen sits at the foot.
 * „Arbeit“ belongs to an event, so it appears only while the path names one.
 */
export function mainItems(eventId: string | undefined): Destination[] {
  return [
    inbox,
    myWork,
    ...(eventId === undefined ? [] : [work(eventId), timeline(eventId), documents(eventId)]),
    eventsItem(eventId),
    persons,
    institutions,
  ];
}
/** The items that do not depend on the event. */
export const MAIN: Destination[] = mainItems(undefined);
export const SETTINGS = settings;
/**
 * The bottom bar on narrow layouts has room for three items and „Mehr“ (ADR 0082). The third is
 * „Anlässe“, and inside an event it is „Arbeit“.
 */
export function bottomItems(eventId: string | undefined): Destination[] {
  return [inbox, myWork, eventId === undefined ? events : work(eventId)];
}
/** „Mehr“ holds the other destinations; inside an event that includes „Anlässe“. */
export function moreItems(eventId: string | undefined): Destination[] {
  return [
    ...(eventId === undefined ? [] : [eventsItem(eventId), documents(eventId)]),
    persons,
    institutions,
    settings,
    privacy,
  ];
}
export const PRIVACY = privacy;
/** Each destination, for the command palette. */
export const ALL: Destination[] = [...MAIN, settings, privacy];

/** True when the path belongs to the destination. */
export function isCurrent(destination: Destination, pathname: string, search = ""): boolean {
  if (yieldsAt(pathname, destination.yieldsTo)) {
    return false;
  }
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

/** The label of the destination that the path belongs to, or nothing for an unknown path. */
export function sectionTitle(pathname: string): string | undefined {
  return ALL.find((destination) => isCurrent(destination, pathname))?.label;
}

/** The ID of the event that the path names, for example `/events/<id>/actions`; not `/events/new`. */
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
