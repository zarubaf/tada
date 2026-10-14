// What the side panel needs to know about a kind of record (ADR 0082): its fields and the commands
// that change them. The frame (`RecordPanel`) has no rule of a kind. A kind joins by one descriptor
// in `descriptors.ts`; it brings no component of its own. The controls are a closed set that every
// kind shares, and the choices of a control (members, areas, milestones) come from the page.
import type { Icon } from "@tabler/icons-react";
import type { ComponentType } from "react";
import type { RecordKind } from "../../api/client";
import type { CommandResult, WorkCommands } from "../workCommands";
import type { WorkItem } from "../workItem";

/** How a field is shown and edited. Each kind uses these and adds none. */
export type ControlKind =
  | "text"
  | "multiline"
  | "date"
  | "status"
  | "enum"
  | "person"
  | "area"
  | "milestone"
  | "people";

/** The icon of a value of an enum: a Tabler icon or a drawing of the same shape. */
export type EnumIcon = ComponentType<{ size?: number; "aria-hidden"?: "true" }>;

/** The text of a field, or the ID of a choice, or a civil date; nothing means the field is empty. */
export type FieldValue = string | null;

/** The IDs of the people of a `people` field. A person or an institution is `person:{uuid}` or `institution:{uuid}`. */
export const peopleIds = (value: FieldValue): string[] =>
  value === null || value === "" ? [] : value.split(",");

/** The value of a `people` field for these IDs: nobody is no value. */
export const peopleValue = (ids: readonly string[]): FieldValue =>
  ids.length === 0 ? null : ids.join(",");

/** What a command returns that the panel reads: the readable ID of a new record. */
export interface Saved {
  local_id: string;
}

export interface FieldSpec {
  /** Stable key of the field, also its key in the values of the create form. */
  id: string;
  /** The Fluent key of the label. */
  labelKey: string;
  /** The icon before the label of a property. The title and the description have none. */
  icon?: Icon;
  control: ControlKind;
  /**
   * The name of the field in the errors of the server (`/due_date`, `/owner`), to place an error at
   * the field. It is not always the name in the request.
   */
  pointer: string;
  /**
   * Other names in the errors of the server that belong to this field as well. The server names
   * the start date for a due date that lies before it, so the due field lists `start_date`. The
   * message for such an error is `work-error-<pointer>-<code>`.
   */
  elsewhere?: readonly string[];
  /**
   * A rule that the client checks before it sends a change: the message if `value` is not allowed
   * for `item`, else nothing. The server checks the same rule again.
   */
  validate?: (item: WorkItem, value: FieldValue) => string | undefined;
  /** The field must have a value. */
  required?: boolean;
  /**
   * The Fluent key of the words for no value, for example „Kein Bereich“. A field with it can be
   * cleared; the words are the choice that clears a picker and the text of an empty field.
   */
  emptyKey?: string;
  /** The Fluent key of the words on a picker that has no choice and cannot be cleared. */
  promptKey?: string;
  /** A date that is marked overdue, as the due date of a task. */
  overdue?: boolean;
  /** The value of the item. */
  read: (item: WorkItem) => FieldValue;
  /**
   * The people that a `people` field shows, with the IDs that its value holds. A field that the
   * member cannot change (the parties of a decision) lists them as text.
   */
  people?: (item: WorkItem) => { id: string; label: string }[];
  /**
   * The values of a status that the item can have now: its own and the next ones. An `enum`
   * control has `values` instead, for they do not depend on the item.
   */
  options?: (item: WorkItem) => string[];
  /**
   * For an `enum` control: the words and the icon of a value. The icon repeats the word and never
   * replaces it.
   */
  present?: (value: string) => { labelKey: string; icon?: EnumIcon };
  /** For an `enum` control: its values, from low to high, in the order that the list shows. */
  values?: readonly string[];
  /** Changes the field. Without it the field is plain text for this kind, whatever the member may. */
  save?: (
    commands: WorkCommands,
    item: WorkItem,
    value: FieldValue,
  ) => Promise<CommandResult<unknown>>;
}

export interface CreateSpec {
  /** The Fluent key of the heading, for example „Neue Aufgabe“. */
  titleKey: string;
  /** The IDs of the fields of the form, in order. The title comes first. */
  fields: readonly string[];
  /**
   * The values that the form starts with. `owner` is the member who is signed in, if they can own
   * a record; otherwise there is no default owner.
   */
  defaults: (owner: string | undefined) => Record<string, FieldValue>;
  /** Checks the values before the form is sent: the message by field ID for each invalid field. */
  validate?: (values: Record<string, FieldValue>) => Record<string, string>;
  /**
   * Creates the record with the ID that the client chose (a UUIDv7). The form sends the same ID
   * on every try, so a retry cannot create the record twice.
   */
  submit: (
    commands: WorkCommands,
    values: Record<string, FieldValue>,
    id: string,
  ) => Promise<CommandResult<Saved>>;
}

export interface RecordDescriptor {
  kind: RecordKind;
  /** The title of the record. It is the heading of the panel. */
  title: FieldSpec;
  /** The fields under the title, in order. */
  properties: readonly FieldSpec[];
  /** The long text under the fields, if the kind has one. */
  description?: FieldSpec;
  /** Further long texts under the description, each in its own section, for example a mitigation. */
  texts?: readonly FieldSpec[];
  /** The Fluent key of the line that names the record version that a passage of evidence supports. */
  evidenceVersionKey: string;
  create?: CreateSpec;
}
