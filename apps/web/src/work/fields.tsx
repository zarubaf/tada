import { t } from "../i18n";
import { Select } from "../ui/Select";
import type { Choice, Directory } from "./directory";

/** The choice „no workstream“: a select cannot hold an empty value. */
export const NO_WORKSTREAM = "none";

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

/** The active workstreams, and the workstream of the record even after it closed. */
export function WorkstreamSelect({
  directory,
  value,
  current,
  onChange,
  error,
}: {
  directory: Directory;
  value: string;
  current: string | null | undefined;
  onChange: (id: string) => void;
  error?: string | undefined;
}) {
  const options: Choice[] = [
    { id: NO_WORKSTREAM, label: t("work-no-workstream") },
    ...directory.workstreams
      .filter((w) => w.status === "active" || w.id === current)
      .map((w) => ({ id: w.id, label: w.name })),
  ];
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
