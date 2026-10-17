// The content of the sidebar: it sits docked on medium and wide layouts and in the drawer on narrow
// ones (doc/design/layout-and-responsiveness.md).
import { type ReactNode, useId } from "react";
import type { SystemVersion } from "../api/client";
import { t } from "../i18n";
import { InboxCount } from "../review/InboxCount";
import { Avatar } from "../ui/Avatar";
import { DisabledNavItem, NavLink } from "../ui/NavLink";
import { type Destination, eventItems, ORGANIZATION_ITEMS, PRIVACY, SETTINGS } from "./navItems";
import styles from "./Sidebar.module.css";
import { VersionButton } from "./VersionFooter";

export interface SidebarProps {
  /** Only a member of more than one organization has it. */
  organizationSwitcher: ReactNode;
  eventSwitcher: ReactNode;
  /** The ID of the element of the switcher that holds the event name: it names the event group. */
  eventNameId: string;
  /** The scoped event: the event items act on it (ADR 0084). */
  eventId: string | undefined;
  server: SystemVersion | undefined;
  onOpenVersions: () => void;
  /** Called when a link starts a navigation: the drawer closes then. */
  onNavigate: () => void;
  member: { id: string; name: string; role: string };
}

export function Sidebar({
  organizationSwitcher,
  eventSwitcher,
  eventNameId,
  eventId,
  server,
  onOpenVersions,
  onNavigate,
  member,
}: SidebarProps) {
  const hintId = useId();
  const link = (destination: Destination) => (
    <NavLink
      key={destination.id}
      onNavigate={onNavigate}
      to={destination.to}
      icon={destination.icon}
      {...(destination.exact ? { exact: true } : {})}
      {...(destination.within ? { within: destination.within } : {})}
      {...(destination.onlyWith ? { onlyWith: destination.onlyWith } : {})}
      {...(destination.unlessWith ? { unlessWith: destination.unlessWith } : {})}
    >
      {destination.label}
      {destination.id === "inbox" && <InboxCount />}
    </NavLink>
  );
  return (
    <section className={styles.sidebar} aria-label={t("shell-sidebar")}>
      <p className={styles.brand}>{t("app-name")}</p>
      {organizationSwitcher}
      <nav className={styles.nav} aria-label={t("shell-nav")}>
        {ORGANIZATION_ITEMS.map(link)}
        <div className={styles.eventHead}>{eventSwitcher}</div>
        {/* The switcher heads the group and names it. A fieldset is for form controls. */}
        {/* biome-ignore lint/a11y/useSemanticElements: a group of links, not of form controls */}
        <div role="group" aria-labelledby={eventNameId} className={styles.eventItems}>
          {eventId === undefined ? (
            <>
              {eventItems("").map((destination) => (
                <DisabledNavItem key={destination.id} icon={destination.icon} describedBy={hintId}>
                  {destination.label}
                </DisabledNavItem>
              ))}
              <p id={hintId} className={styles.hint}>
                {t("nav-needs-event")}
              </p>
            </>
          ) : (
            eventItems(eventId).map(link)
          )}
        </div>
      </nav>
      <div className={styles.foot}>
        <NavLink
          to={SETTINGS.to}
          onNavigate={onNavigate}
          icon={SETTINGS.icon}
          within={SETTINGS.within}
        >
          {SETTINGS.label}
        </NavLink>
        <NavLink to={PRIVACY.to} onNavigate={onNavigate} icon={PRIVACY.icon}>
          {PRIVACY.label}
        </NavLink>
        <VersionButton server={server} onPress={onOpenVersions} />
        <div className={styles.member}>
          <Avatar name={member.name} id={member.id} size={24} decorative />
          <p className={styles.who}>
            <span className={styles.name}>{member.name}</span>
            <span className={styles.role}>{member.role}</span>
          </p>
        </div>
      </div>
    </section>
  );
}
