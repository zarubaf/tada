import { useState } from "react";
import type { Milestone } from "../api/client";
import { t } from "../i18n";
import { ComboBox } from "../ui/ComboBox";
import { Select } from "../ui/Select";
import type { Choice, Directory } from "./directory";
import { milestoneName } from "./milestoneName";

/** The choice „no workstream“: a select cannot hold an empty value. */
export const NO_WORKSTREAM = "none";

/** The choice „no milestone“, for the same reason. */
export const NO_MILESTONE = "none";

/** The members that can own a record, and the current owner even if the member cannot choose them. */
export function OwnerSelect({
  directory,
  value,
  current,
  label,
  onChange,
  error,
}: {
  directory: Directory;
  value: string;
  /** The saved owner of the record that the form changes. */
  current: string | undefined;
  label: string;
  onChange: (id: string) => void;
  error?: string | undefined;
}) {
  const options: Choice[] = [...directory.assignees];
  if (current !== undefined && !options.some((option) => option.id === current)) {
    options.push({ id: current, label: directory.nameOf(current) });
  }
  return <Select label={label} options={options} value={value} onChange={onChange} error={error} />;
}

/**
 * The active workstreams, and the workstream of the record even after it closed.
 * A workstream that the directory does not list yet, for example a new one of a changeset, shows
 * as `currentLabel`.
 */
export function WorkstreamSelect({
  directory,
  value,
  current,
  currentLabel,
  onChange,
  error,
}: {
  directory: Directory;
  value: string;
  current: string | null | undefined;
  currentLabel?: string | undefined;
  onChange: (id: string) => void;
  error?: string | undefined;
}) {
  const options: Choice[] = [
    { id: NO_WORKSTREAM, label: t("work-no-workstream") },
    ...directory.workstreams
      .filter((w) => w.status === "active" || w.id === current)
      .map((w) => ({ id: w.id, label: w.name })),
  ];
  if (current && currentLabel && !options.some((option) => option.id === current)) {
    options.push({ id: current, label: currentLabel });
  }
  return (
    <Select
      label={t("work-field-workstream")}
      options={options}
      value={value}
      onChange={onChange}
      error={error}
    />
  );
}

/**
 * The milestones that can take an action, as a search: an event often has more than seven. All but
 * the canceled ones; the milestone of the record stays a choice even after it was canceled, and
 * `currentName` names it if the list lacks it, for example a milestone that the same changeset
 * creates. „Kein Meilenstein“ is the empty choice, also when the member clears the input.
 */
export function MilestoneSelect({
  milestones,
  value,
  current,
  currentName,
  onChange,
  error,
}: {
  milestones: Milestone[];
  value: string;
  current: string | null | undefined;
  currentName?: string | undefined;
  onChange: (id: string) => void;
  error?: string | undefined;
}) {
  const options: Choice[] = [
    { id: NO_MILESTONE, label: t("work-no-milestone") },
    ...milestones
      .filter((m) => m.status !== "canceled" || m.id === current)
      .map((m) => ({ id: m.id, label: milestoneName(m) })),
  ];
  if (current && !options.some((option) => option.id === current)) {
    options.push({ id: current, label: currentName ?? t("inbox-name-unknown") });
  }
  const chosen = options.find((option) => option.id === value)?.label ?? "";
  // The input shows the chosen milestone; a new value from outside, for example after a save,
  // shows its name again.
  const [text, setText] = useState(chosen);
  const [shownFor, setShownFor] = useState(value);
  if (shownFor !== value) {
    setShownFor(value);
    setText(chosen);
  }
  // The name of the chosen milestone is no search: the list then shows every choice.
  const query = text === chosen ? "" : text.trim().toLocaleLowerCase("de-CH");
  const matches = options.filter((option) =>
    option.label.toLocaleLowerCase("de-CH").includes(query),
  );
  return (
    <ComboBox
      label={t("work-field-milestone")}
      options={matches}
      inputValue={text}
      onInputChange={setText}
      selectedKey={value}
      onSelectionChange={(id) => {
        if (id === undefined) {
          // A cleared input keeps the text of the member, who is about to type a search.
          setShownFor(NO_MILESTONE);
        }
        onChange(id ?? NO_MILESTONE);
      }}
      error={error}
    />
  );
}
