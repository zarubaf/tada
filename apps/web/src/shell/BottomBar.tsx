// The bottom bar on narrow layouts (ADR 0082): Eingang, Meine Arbeit, Anlässe (inside an event:
// Arbeit) and „Mehr“, a menu with the other destinations.
import { IconDots } from "@tabler/icons-react";
import { t } from "../i18n";
import { InboxCount } from "../review/InboxCount";
import { useNavigate, usePathname } from "../router/Router";
import { MenuButton } from "../ui/MenuButton";
import { NavLink } from "../ui/NavLink";
import styles from "./BottomBar.module.css";
import { bottomItems, eventIdOf, isCurrent, moreItems } from "./navItems";

const VERSIONS = "versions";

export function BottomBar({
  versionsLabel,
  onOpenVersions,
}: {
  /** The label of the item that opens the list of the components. */
  versionsLabel: string;
  onOpenVersions: () => void;
}) {
  const navigate = useNavigate();
  const pathname = usePathname();
  const eventId = eventIdOf(pathname);
  const more = moreItems(eventId);
  return (
    <nav className={styles.bar} aria-label={t("shell-nav")}>
      {bottomItems(eventId).map((destination) => (
        <NavLink
          key={destination.id}
          to={destination.to}
          icon={destination.icon}
          large
          {...(destination.exact ? { exact: true } : {})}
        >
          <span className={styles.label}>{destination.label}</span>
          {destination.id === "inbox" && (
            <span className={styles.badge}>
              <InboxCount />
            </span>
          )}
        </NavLink>
      ))}
      <MenuButton
        trigger={
          <>
            <IconDots size={16} stroke={1.5} aria-hidden="true" />
            <span className={styles.label}>{t("nav-more")}</span>
          </>
        }
        triggerLabel={t("nav-more")}
        triggerClassName={styles.more}
        current={more.some((destination) => isCurrent(destination, pathname))}
        label={t("nav-more-menu")}
        items={[
          ...more.map((destination) => ({
            id: destination.to,
            label: destination.label,
            icon: destination.icon,
          })),
          { id: VERSIONS, label: versionsLabel },
        ]}
        onAction={(id) => (id === VERSIONS ? onOpenVersions() : navigate(id))}
        placement="top end"
      />
    </nav>
  );
}
