// The command palette behind the command box (ADR 0082). It navigates to the destinations of the
// shell, jumps to an event, starts a new record of each kind that the member may create in the
// current event and switches the theme. It searches no records yet. This module loads on the first
// opening, so that the first load stays small (ADR 0024).
import { IconCalendarEvent, IconCircleHalf2, IconFilter, IconPlus } from "@tabler/icons-react";
import { useEffect } from "react";
import type { Api } from "../api/client";
import { t } from "../i18n";
import { useNavigate } from "../router/Router";
import { chooseTheme, THEMES } from "../theme/theme";
import Palette, { type CommandGroup } from "../ui/CommandPalette";
import { manageStreamsAddress } from "../work/manageStreams";
import { useCreatableKinds } from "../work/useCreatableKinds";
import { NEW_EVENT } from "./EventSwitcher";
import { paletteDestinations } from "./navItems";
import { useSwitchEvent } from "./switchEvent";
import type { EventList } from "./useEvents";

const GOTO = "goto:";
const EVENT = "event:";
const THEME = "theme:";
const NEW_RECORD = "new-record:";
const MANAGE_STREAMS = "manage-streams";

interface Props {
  api: Api;
  events: EventList;
  loadEvents: () => void;
  /** The event of the route: the record actions act on it. */
  routeEventId: string | undefined;
  /** The scoped event (ADR 0084): the event destinations act on it, on every page. */
  scopedEventId: string | undefined;
  /** False for a closed event, which is read-only for everybody. */
  eventWritable: boolean;
  canCreateEvent: boolean;
  onClose: () => void;
}

/**
 * The palette of the shell. One component stays mounted from the opening, so that the typed text
 * survives the moment when the current event becomes known.
 */
export default function CommandPalette({
  api,
  events,
  loadEvents,
  routeEventId,
  scopedEventId,
  eventWritable,
  canCreateEvent,
  onClose,
}: Props) {
  const navigate = useNavigate();
  const switchEvent = useSwitchEvent();
  const creatable = useCreatableKinds(api, routeEventId, eventWritable);
  // biome-ignore lint/correctness/useExhaustiveDependencies: the list loads once, when the palette opens
  useEffect(loadEvents, []);

  const groups: CommandGroup[] = [
    {
      id: "goto",
      label: t("command-group-goto"),
      commands: paletteDestinations(scopedEventId).map((destination) => ({
        id: `${GOTO}${destination.to}`,
        label: destination.label,
        icon: destination.icon,
      })),
    },
    {
      id: "actions",
      label: t("command-group-actions"),
      commands: [
        ...creatable.map((kind) => ({
          id: `${NEW_RECORD}${kind}`,
          label: t(`record-create-${kind}`),
          icon: IconPlus,
        })),
        // Every member of the event reads the workstreams; the dialog offers what each may change.
        ...(routeEventId === undefined
          ? []
          : [{ id: MANAGE_STREAMS, label: t("workstreams-manage-title"), icon: IconFilter }]),
        ...(canCreateEvent
          ? [{ id: NEW_EVENT, label: t("command-new-event"), icon: IconPlus }]
          : []),
        ...THEMES.map((theme) => ({
          id: `${THEME}${theme}`,
          label: t("theme-toggle", { name: t(`theme-${theme}`) }),
          icon: IconCircleHalf2,
        })),
      ],
    },
    {
      id: "events",
      label: t("command-group-events"),
      commands:
        events.kind === "loaded"
          ? events.events.map((event) => ({
              id: `${EVENT}${event.id}`,
              label: event.name,
              icon: IconCalendarEvent,
            }))
          : [],
    },
  ];

  const run = (id: string) => {
    if (id.startsWith(GOTO)) {
      navigate(id.slice(GOTO.length));
    } else if (id.startsWith(EVENT)) {
      switchEvent(id.slice(EVENT.length));
    } else if (id.startsWith(THEME)) {
      chooseTheme(id.slice(THEME.length) as (typeof THEMES)[number]);
    } else if (id.startsWith(NEW_RECORD) && routeEventId !== undefined) {
      const kind = id.slice(NEW_RECORD.length);
      navigate(`/events/${encodeURIComponent(routeEventId)}/work?new=${kind}`);
    } else if (id === MANAGE_STREAMS && routeEventId !== undefined) {
      navigate(manageStreamsAddress(routeEventId));
    } else if (id === NEW_EVENT) {
      navigate("/events/new");
    }
    onClose();
  };

  return (
    <Palette
      label={t("command-palette-label")}
      placeholder={t("command-palette-search")}
      groups={groups}
      onAction={run}
      onClose={onClose}
    />
  );
}
