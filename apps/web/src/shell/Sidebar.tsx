// The content of the sidebar: it sits docked on medium and wide layouts and in the drawer on narrow
// ones (doc/design/layout-and-responsiveness.md).
import type { ReactNode } from "react";
import type { SystemVersion } from "../api/client";
import { t } from "../i18n";
import { InboxCount } from "../review/InboxCount";
import { Avatar } from "../ui/Avatar";
import { NavLink } from "../ui/NavLink";
import { MAIN, PRIVACY } from "./navItems";
import styles from "./Sidebar.module.css";
import { VersionButton } from "./VersionFooter";

export interface SidebarProps {
  /** Only a member of more than one organization has it. */
  organizationSwitcher: ReactNode;
  eventSwitcher: ReactNode;
  server: SystemVersion | undefined;
  onOpenVersions: () => void;
  member: { id: string; name: string; role: string };
}

export function Sidebar({
  organizationSwitcher,
  eventSwitcher,
  server,
  onOpenVersions,
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
        <NavLink to={PRIVACY.to} icon={PRIVACY.icon}>
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
