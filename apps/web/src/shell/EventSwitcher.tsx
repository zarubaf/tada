// The event switcher at the head of the Anlass group (ADR 0084), or in the top bar when the sidebar is not docked: the current event with its key, and a
// menu with the events of the member, „Alle Anlässe“ and, for the administration, „Neuer Anlass“.
import { IconCalendarEvent, IconPlus, IconSelector } from "@tabler/icons-react";
import type { Event } from "../api/client";
import { t } from "../i18n";
import { initialsOf } from "../ui/Avatar";
import { MenuButton, type MenuButtonItem } from "../ui/MenuButton";
import styles from "./EventSwitcher.module.css";
import type { EventList } from "./useEvents";

export const ALL_EVENTS = "all";
export const NEW_EVENT = "new";
const EVENT_PREFIX = "event:";

/** The menu item ID of an event, and back. */
const eventItemId = (id: string) => `${EVENT_PREFIX}${id}`;
export const eventIdOfItem = (item: string) =>
  item.startsWith(EVENT_PREFIX) ? item.slice(EVENT_PREFIX.length) : undefined;

/** The key of the event; a closed event says so. */
function subtitle(event: Event): string {
  return event.status === "closed" ? `${event.key} · ${t("event-switcher-closed")}` : event.key;
}

export function EventSwitcher({
  current,
  list,
  canCreate,
  onOpen,
  onAction,
  nameId,
  inTopBar,
}: {
  current: Event | undefined;
  list: EventList;
  canCreate: boolean;
  onOpen: () => void;
  onAction: (item: string) => void;
  /** The ID of the name, for a group that the switcher heads. */
  nameId?: string;
  /** The switcher of the top bar: the tile and the chevron only on narrow layouts. */
  inTopBar?: boolean;
}) {
  const items: MenuButtonItem[] = [
    ...(list.kind === "loaded"
      ? list.events.map((event) => ({
          id: eventItemId(event.id),
          label: event.name,
          ...(event.id === current?.id
            ? { current: true, ariaLabel: t("event-switcher-current-item", { name: event.name }) }
            : {}),
        }))
      : []),
    ...(list.kind === "failed"
      ? [{ id: "failed", label: t("event-switcher-failed"), isDisabled: true }]
      : []),
    ...(list.kind === "loading" || list.kind === "idle"
      ? [{ id: "loading", label: t("event-switcher-loading"), isDisabled: true }]
      : []),
    { id: ALL_EVENTS, label: t("event-switcher-all"), icon: IconCalendarEvent, separated: true },
    ...(canCreate ? [{ id: NEW_EVENT, label: t("event-switcher-create"), icon: IconPlus }] : []),
  ];

  return (
    <MenuButton
      trigger={
        <>
          <span className={styles.tile} aria-hidden="true">
            {current ? initialsOf(current.name) : "?"}
          </span>
          <span className={styles.text}>
            {/* A long name is cut: the full name is the title and in the name of the button. */}
            <span id={nameId} className={styles.name} title={current?.name}>
              {current?.name ?? t("event-switcher-pick")}
            </span>
            {current && (
              <span className={styles.sub} title={subtitle(current)}>
                {subtitle(current)}
              </span>
            )}
          </span>
          <IconSelector size={16} stroke={1.5} aria-hidden="true" className={styles.chevron} />
        </>
      }
      triggerLabel={
        current ? t("event-switcher-label", { name: current.name }) : t("event-switcher-pick")
      }
      triggerClassName={inTopBar ? `${styles.trigger} ${styles.topBar}` : styles.trigger}
      label={t("event-switcher-menu")}
      items={items}
      onAction={onAction}
      onOpen={onOpen}
      placement="bottom start"
    />
  );
}
