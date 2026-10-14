// The command palette behind the command box (ADR 0082). It navigates to the destinations of the
// shell, jumps to an event, starts a new action in the current event and switches the theme. It
// searches no records yet. This module loads on the first opening, so that the first load stays
// small (ADR 0024).
import { IconCalendarEvent, IconCircleHalf2, IconPlus } from "@tabler/icons-react";
import { useEffect } from "react";
import { t } from "../i18n";
import { useNavigate } from "../router/Router";
import { chooseTheme, THEMES } from "../theme/theme";
import Palette, { type CommandGroup } from "../ui/CommandPalette";
import { NEW_EVENT } from "./EventSwitcher";
import { ALL } from "./navItems";
import type { EventList } from "./useEvents";

const GOTO = "goto:";
const EVENT = "event:";
const THEME = "theme:";
const NEW_ACTION = "new-action";

export default function CommandPalette({
  events,
  loadEvents,
  currentEventId,
  canCreateEvent,
  onClose,
}: {
  events: EventList;
  loadEvents: () => void;
  currentEventId: string | undefined;
  canCreateEvent: boolean;
  onClose: () => void;
}) {
  const navigate = useNavigate();
  // biome-ignore lint/correctness/useExhaustiveDependencies: the list loads once, when the palette opens
  useEffect(loadEvents, []);

  const groups: CommandGroup[] = [
    {
      id: "goto",
      label: t("command-group-goto"),
      commands: ALL.map((destination) => ({
        id: `${GOTO}${destination.to}`,
        label: destination.label,
        icon: destination.icon,
      })),
    },
    {
      id: "actions",
      label: t("command-group-actions"),
      commands: [
        ...(currentEventId === undefined
          ? []
          : [{ id: NEW_ACTION, label: t("command-new-action"), icon: IconPlus }]),
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
      navigate(`/events/${encodeURIComponent(id.slice(EVENT.length))}`);
    } else if (id.startsWith(THEME)) {
      chooseTheme(id.slice(THEME.length) as (typeof THEMES)[number]);
    } else if (id === NEW_ACTION && currentEventId !== undefined) {
      navigate(`/events/${encodeURIComponent(currentEventId)}/work?new=action`);
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
