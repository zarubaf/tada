// The choices of the pickers of the panel: members, areas, milestones and the next statuses. They
// come from the page, so a kind brings none. A picker is offered only while its list is complete;
// before that, and without the right to change, it shows plain text.
import type { Api, Milestone } from "../../api/client";
import { t } from "../../i18n";
import { Avatar } from "../../ui/Avatar";
import type { SearchOption } from "../../ui/SearchList";
import { StatusIcon } from "../../ui/StatusIcon";
import type { Directory } from "../directory";
import { milestoneName } from "../milestoneName";
import { statusPresentationOfText } from "../recordStatus";
import type { WorkItem } from "../workItem";
import { type FieldSpec, type FieldValue, REDACTED, type RecordLookup } from "./descriptor";
import { enumChoices } from "./enumChoices";

/** What the choices are made of. A part that has not loaded is absent. */
export interface ChoiceSources {
  directory: Directory | undefined;
  /** All milestones of the event. */
  milestones: Milestone[] | undefined;
  /** The readable IDs of the records that a field refers to. */
  records?: RecordLookup | undefined;
  /** For a control that searches on the server, such as the people of a decision. */
  api?: Api | undefined;
}

export interface Choices {
  options: SearchOption[];
  /** True if the list is complete, so that the member can choose from it. */
  offered: boolean;
}

/** The choice for the current value is always there, also when the list would not offer it. */
function withCurrent(
  options: SearchOption[],
  current: FieldValue,
  fallback: SearchOption,
): SearchOption[] {
  return current === null || options.some((option) => option.id === current)
    ? options
    : [...options, fallback];
}

/**
 * The choices of a field. `item` is the record that the panel shows, if there is one, and
 * `current` the value that the field shows now. Text and date controls have none.
 */
export function choicesOf(
  spec: FieldSpec,
  sources: ChoiceSources,
  item: WorkItem | undefined,
  current: FieldValue,
): Choices {
  const { directory, milestones } = sources;
  switch (spec.control) {
    case "status": {
      // The statuses are the ones of a record, so a form for a new record has none.
      if (!item || !spec.options) {
        return { options: [], offered: false };
      }
      const statuses = spec.options(item);
      return {
        options: statuses.map((status) => {
          const { glyph, tone, labelKey } = statusPresentationOfText(item.kind, status);
          return {
            id: status,
            label: t(labelKey),
            leading: <StatusIcon glyph={glyph} tone={tone} size={14} />,
          };
        }),
        offered: statuses.length > 1,
      };
    }
    case "enum":
      return enumChoices(spec);
    case "person": {
      const options = (directory?.assignees ?? []).map((member) => ({
        id: member.id,
        label: member.label,
        leading: <Avatar name={member.label} id={member.id} decorative />,
      }));
      const name = (id: string) => directory?.nameOf(id) || item?.owner.name || "";
      return {
        options: withCurrent(options, current, {
          id: current ?? "",
          label: name(current ?? ""),
          leading: <Avatar name={name(current ?? "")} id={current ?? ""} decorative />,
        }),
        offered: directory !== undefined,
      };
    }
    case "area": {
      const options = (directory?.workstreams ?? [])
        .filter((stream) => stream.status === "active" || stream.id === current)
        .map((stream) => ({ id: stream.id, label: stream.name }));
      return {
        options: withCurrent(options, current, {
          id: current ?? "",
          label: t("record-unknown-area"),
        }),
        offered: directory !== undefined,
      };
    }
    case "milestone": {
      const options = (milestones ?? [])
        .filter((milestone) => milestone.status !== "canceled" || milestone.id === current)
        .map((milestone) => ({ id: milestone.id, label: milestoneName(milestone) }));
      return {
        options: withCurrent(options, current, {
          id: current ?? "",
          label: t("record-unknown-milestone"),
        }),
        offered: milestones !== undefined,
      };
    }
    case "record": {
      // The record that the field refers to, named by its readable ID once its kind has loaded.
      const label =
        current && spec.refers ? sources.records?.readableId(spec.refers, current) : undefined;
      return { options: label ? [{ id: current ?? "", label }] : [], offered: true };
    }
    case "people": {
      // The people are those of the record; the search of the form needs no options here.
      const people = item && spec.people ? spec.people(item) : [];
      return {
        options: people.map((person) =>
          person.label === REDACTED
            ? { id: person.id, label: t("redacted") }
            : {
                id: person.id,
                label: person.label,
                leading: <Avatar name={person.label} id={person.id} decorative />,
              },
        ),
        offered: true,
      };
    }
    default:
      return { options: [], offered: true };
  }
}
