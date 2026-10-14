// The destinations of the shell, once: the sidebar, the bottom bar, the menu „Mehr“ and the command
// palette all read this list. An item appears only when its page exists (ADR 0082).
import {
  type Icon,
  IconBuilding,
  IconCalendarEvent,
  IconInbox,
  IconSettings,
  IconShieldLock,
  IconUser,
  IconUsers,
} from "@tabler/icons-react";
import { t } from "../i18n";

export interface Destination {
  id: string;
  to: string;
  label: string;
  icon: Icon;
  /** Current only for its own path. */
  exact?: boolean;
  /** Current also below this prefix, when the path of the link is not a prefix of its pages. */
  within?: string;
}

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

/** The items of the sidebar and of the drawer, from top to bottom. */
export const MAIN: Destination[] = [inbox, myWork, events, persons, institutions, settings];
/** The bottom bar on narrow layouts has room for these three and „Mehr“. */
export const BOTTOM: Destination[] = [inbox, myWork, events];
/** „Mehr“ holds the other destinations. */
export const MORE: Destination[] = [persons, institutions, settings, privacy];
export const PRIVACY = privacy;
/** Each destination, for the command palette. */
export const ALL: Destination[] = [...MAIN, privacy];

function isCurrent(destination: Destination, pathname: string): boolean {
  return (
    pathname === destination.to ||
    (!destination.exact && pathname.startsWith(`${destination.to}/`)) ||
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
