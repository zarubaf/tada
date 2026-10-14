// The filter controls of the „Arbeit“ page, one component each, so that the toolbar (wide) and the
// filter sheet (narrow) place the same controls. They only read and change `WorkFilters`.
import { IconX } from "@tabler/icons-react";
import { type Ref, useMemo } from "react";
import { t } from "../i18n";
import { Avatar } from "../ui/Avatar";
import { Button } from "../ui/Button";
import { SearchField } from "../ui/SearchField";
import { type SearchOption, SearchSelect } from "../ui/SearchSelect";
import { Select } from "../ui/Select";
import type { DirectoryState } from "./directory";
import { useDebouncedText } from "./useDebouncedText";
import { DUE_WINDOWS, type DueWindow, type WorkFilters } from "./workFilters";

export interface FilterControlProps {
  filters: WorkFilters;
  onChange: (next: WorkFilters) => void;
}

/** True when a filter other than the default is set. */
export function isFiltered(filters: WorkFilters): boolean {
  return filters.kinds.length > 0 || countOtherFilters(filters) > 0;
}

/** The filters of the sheet: all except the type, which has its own control. */
export function countOtherFilters(filters: WorkFilters): number {
  return [
    filters.text !== "",
    filters.owners.length > 0,
    filters.workstreams.length > 0,
    filters.due !== undefined,
  ].filter(Boolean).length;
}

export function SearchBox({
  filters,
  onChange,
  inputRef,
}: FilterControlProps & { inputRef: Ref<HTMLInputElement> }) {
  // The field keeps what the member types, with its spaces; the filters get the text after a pause.
  const field = useDebouncedText(filters.text, (text) => onChange({ ...filters, text }));
  return (
    <SearchField
      label={t("work-search")}
      placeholder={t("work-search-placeholder")}
      value={field.draft}
      onChange={field.change}
      onCommit={field.flush}
      inputRef={inputRef}
    />
  );
}

const MINE = "mine";

export function OwnerFilter({
  filters,
  onChange,
  directory,
  me,
}: FilterControlProps & { directory: DirectoryState; me: string }) {
  // „Ich“ first, then the others. The owner is required on an action, so there is no „Niemand“.
  const options = useMemo<SearchOption[]>(() => {
    const others =
      directory.kind === "loaded"
        ? directory.directory.assignees.filter((member) => member.id !== me)
        : [];
    return [
      { id: MINE, label: t("work-owner-me") },
      ...others.map((member) => ({
        id: member.id,
        label: member.label,
        leading: <Avatar name={member.label} id={member.id} decorative />,
      })),
    ];
  }, [directory, me]);
  const [first] = filters.owners;
  return (
    <SearchSelect
      label={t("work-owner")}
      placeholder={t("work-owner-any")}
      clearLabel={t("work-owner-any")}
      options={options}
      value={first === me ? MINE : first}
      onChange={(id) => onChange({ ...filters, owners: id === undefined ? [] : [id] })}
    />
  );
}

export function StreamFilter({
  filters,
  onChange,
  directory,
}: FilterControlProps & { directory: DirectoryState }) {
  const options = useMemo<SearchOption[]>(
    () =>
      directory.kind === "loaded"
        ? directory.directory.workstreams.map((stream) => ({ id: stream.id, label: stream.name }))
        : [],
    [directory],
  );
  return (
    <SearchSelect
      label={t("work-stream")}
      placeholder={t("work-stream-any")}
      clearLabel={t("work-stream-any")}
      options={options}
      value={filters.workstreams[0]}
      onChange={(id) => onChange({ ...filters, workstreams: id === undefined ? [] : [id] })}
    />
  );
}

const ANY_DUE = "any";

export function DueFilter({ filters, onChange }: FilterControlProps) {
  const options = [
    { id: ANY_DUE, label: t("work-due-any") },
    ...DUE_WINDOWS.map((window) => ({ id: window, label: t(`work-due-${window}`) })),
  ];
  return (
    <Select
      label={t("work-due")}
      labelHidden
      options={options}
      value={filters.due ?? ANY_DUE}
      onChange={(id) =>
        onChange({ ...filters, due: id === ANY_DUE ? undefined : (id as DueWindow) })
      }
    />
  );
}

export function ResetFilters({ filters, onReset }: { filters: WorkFilters; onReset: () => void }) {
  if (!isFiltered(filters)) {
    return null;
  }
  return (
    <Button variant="quiet" onPress={onReset}>
      <IconX size={14} stroke={1.5} aria-hidden="true" />
      {t("work-filter-reset")}
    </Button>
  );
}
