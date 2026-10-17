// The bottom bar on narrow layouts (ADR 0084): Eingang, Meine Arbeit, Arbeit of the scoped event
// and „Mehr“, a menu with the other destinations. It is the same in every state: with no event the
// event items are off, as in the sidebar.
import { IconDots } from "@tabler/icons-react";
import { useEffect, useId, useState } from "react";
import { t } from "../i18n";
import { InboxCount } from "../review/InboxCount";
import { useNavigate, usePathname } from "../router/Router";
import { MenuButton } from "../ui/MenuButton";
import { DisabledNavItem, NavLink } from "../ui/NavLink";
import styles from "./BottomBar.module.css";
import { bottomItems, hasTarget, isCurrent, moreItems } from "./navItems";

const VERSIONS = "versions";

export function BottomBar({
  eventId,
  eventPending,
  versionsLabel,
  onOpenVersions,
}: {
  /** The scoped event that the bar acts on. */
  eventId: string | undefined;
  /** The scoped event is still on its way: the bar keeps its shape and says nothing yet. */
  eventPending: boolean;
  /** The label of the item that opens the list of the components. */
  versionsLabel: string;
  onOpenVersions: () => void;
}) {
  const navigate = useNavigate();
  const pathname = usePathname();
  const hintId = useId();
  // The bar has no room for a line of text: a press on the inert „Arbeit“ shows the reason. It stays
  // until the next press, Escape, the loss of focus or a change of the page or of the event.
  const [hintShown, setHintShown] = useState(false);
  // biome-ignore lint/correctness/useExhaustiveDependencies: the page and the event are the triggers
  useEffect(() => setHintShown(false), [pathname, eventId]);
  useEffect(() => {
    if (!hintShown) {
      return;
    }
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setHintShown(false);
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [hintShown]);
  const more = moreItems(eventId);
  const hint = eventPending ? {} : { describedBy: hintId };
  return (
    <nav className={styles.bar} aria-label={t("shell-nav")}>
      {bottomItems(eventId).map((destination) =>
        hasTarget(destination) ? (
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
        ) : (
          <DisabledNavItem
            key={destination.id}
            icon={destination.icon}
            large
            onPress={() => setHintShown((shown) => !shown)}
            onBlur={() => setHintShown(false)}
            {...hint}
          >
            <span className={styles.label}>{destination.label}</span>
          </DisabledNavItem>
        ),
      )}
      {!eventPending && eventId === undefined && (
        <p id={hintId} className={hintShown ? styles.hintShown : styles.hint}>
          {t("nav-needs-event")}
        </p>
      )}
      <MenuButton
        trigger={
          <>
            <IconDots size={16} stroke={1.5} aria-hidden="true" />
            <span className={styles.label}>{t("nav-more")}</span>
          </>
        }
        triggerLabel={t("nav-more")}
        triggerClassName={styles.more}
        current={more.some((item) => hasTarget(item) && isCurrent(item, pathname))}
        label={t("nav-more-menu")}
        items={[
          ...more.map((item) =>
            hasTarget(item)
              ? { id: item.to, label: item.label, icon: item.icon }
              : {
                  id: item.id,
                  label: item.label,
                  icon: item.icon,
                  isDisabled: true,
                  ...(eventPending ? {} : { description: t("nav-needs-event") }),
                },
          ),
          { id: VERSIONS, label: versionsLabel },
        ]}
        onAction={(id) => (id === VERSIONS ? onOpenVersions() : navigate(id))}
        placement="top end"
      />
    </nav>
  );
}
