// The command palette behind the command box (ADR 0082). It navigates to the destinations of the
// shell, jumps to an event, starts a new record of each kind that the member may create in the
// current event and switches the theme. It searches no records yet. This module loads on the first
// opening, so that the first load stays small (ADR 0024).
import { IconCalendarEvent, IconCircleHalf2, IconPlus } from "@tabler/icons-react";
import { useEffect } from "react";
import type { Api } from "../api/client";
import { t } from "../i18n";
import { useNavigate } from "../router/Router";
import { chooseTheme, THEMES } from "../theme/theme";
import Palette, { type CommandGroup } from "../ui/CommandPalette";
import { useCreatableKinds } from "../work/useCreatableKinds";
import { NEW_EVENT } from "./EventSwitcher";
import { ALL } from "./navItems";
import type { EventList } from "./useEvents";

const GOTO = "goto:";
const EVENT = "event:";
const THEME = "theme:";
const NEW_RECORD = "new-record:";

interface Props {
  api: Api;
  events: EventList;
  loadEvents: () => void;
  currentEventId: string | undefined;
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
  currentEventId,
  eventWritable,
  canCreateEvent,
  onClose,
}: Props) {
  const navigate = useNavigate();
  const creatable = useCreatableKinds(api, currentEventId, eventWritable);
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
        ...creatable.map((kind) => ({
          id: `${NEW_RECORD}${kind}`,
          label: t(`record-create-${kind}`),
          icon: IconPlus,
        })),
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
    } else if (id.startsWith(NEW_RECORD) && currentEventId !== undefined) {
      const kind = id.slice(NEW_RECORD.length);
      navigate(`/events/${encodeURIComponent(currentEventId)}/work?new=${kind}`);
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
