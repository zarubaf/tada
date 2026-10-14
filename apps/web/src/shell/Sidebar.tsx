// The content of the sidebar: it sits docked on medium and wide layouts and in the drawer on narrow
// ones (doc/design/layout-and-responsiveness.md).
import type { ReactNode } from "react";
import type { SystemVersion } from "../api/client";
import { t } from "../i18n";
import { InboxCount } from "../review/InboxCount";
import { Avatar } from "../ui/Avatar";
import { NavLink } from "../ui/NavLink";
import { MAIN, PRIVACY, SETTINGS } from "./navItems";
import styles from "./Sidebar.module.css";
import { VersionButton } from "./VersionFooter";

export interface SidebarProps {
  /** Only a member of more than one organization has it. */
  organizationSwitcher: ReactNode;
  eventSwitcher: ReactNode;
  server: SystemVersion | undefined;
  onOpenVersions: () => void;
  /** Called when a link starts a navigation: the drawer closes then. */
  onNavigate: () => void;
  member: { id: string; name: string; role: string };
}

export function Sidebar({
  organizationSwitcher,
  eventSwitcher,
  server,
  onOpenVersions,
  onNavigate,
  member,
}: SidebarProps) {
  return (
    <div className={styles.sidebar}>
      <p className={styles.brand}>{t("app-name")}</p>
      {organizationSwitcher}
      {eventSwitcher}
      <nav className={styles.nav} aria-label={t("shell-nav")}>
        {MAIN.map((destination) => (
          <NavLink
            key={destination.id}
            onNavigate={onNavigate}
            to={destination.to}
            icon={destination.icon}
            {...(destination.exact ? { exact: true } : {})}
            {...(destination.within ? { within: destination.within } : {})}
          >
            {destination.label}
            {destination.id === "inbox" && <InboxCount />}
          </NavLink>
        ))}
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
    </div>
  );
}
