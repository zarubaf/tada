import { IconFilter, IconPlus } from "@tabler/icons-react";
import { type ReactNode, type Ref, useState } from "react";
import type { RecordKind } from "../api/client";
import { t } from "../i18n";
import { Button } from "../ui/Button";
import { Select } from "../ui/Select";
import { Sheet } from "../ui/Sheet";
import { TypeFilter } from "../ui/TypeFilter";
import { TypeIcon } from "../ui/TypeIcon";
import type { DirectoryState } from "./directory";
import { RECORD_KINDS } from "./recordStatus";
import type { DebouncedText } from "./useDebouncedText";
import {
  countOtherFilters,
  DueFilter,
  OwnerFilter,
  ResetFilters,
  SearchBox,
  StreamFilter,
  useSearchText,
} from "./WorkFilterControls";
import styles from "./WorkToolbar.module.css";
import type { Counts, WorkFilters } from "./workFilters";

const ALL = "all";

export interface WorkToolbarProps {
  filters: WorkFilters;
  onChange: (next: WorkFilters) => void;
  /** The counts of the loaded work; nothing while it loads. */
  counts: Counts | undefined;
  directory: DirectoryState;
  /** The ID of the signed-in member. */
  me: string;
  /** The `h1` of the page: it sits at the left of the row. */
  title: ReactNode;
  /** Below 40rem the type is a select in a second row (ADR 0023). */
  narrow: boolean;
  /** Clears all filters. The page moves focus afterwards, because the reset button leaves. */
  onReset: () => void;
  /** The search field that shows, for the focus after a reset. */
  searchRef: Ref<HTMLInputElement>;
  /** Opens the form of a new record. Without it the member may not create one, and there is no button. */
  onCreate?: (() => void) | undefined;
}

function NewButton({ onCreate }: { onCreate: () => void }) {
  return (
    <Button variant="primary" size="sm" onPress={onCreate}>
      <IconPlus size={14} stroke={1.5} aria-hidden="true" />
      {t("work-new")}
    </Button>
  );
}

function typeOptions(counts: Counts | undefined) {
  const total = counts && RECORD_KINDS.reduce((sum, kind) => sum + counts.kinds[kind], 0);
  return [
    { id: ALL, label: t("work-type-all"), ...(total === undefined ? {} : { count: total }) },
    ...RECORD_KINDS.map((kind) => ({
      id: kind,
      label: t(`work-type-${kind}`),
      icon: <TypeIcon kind={kind} size={14} decorative />,
      ...(counts ? { count: counts.kinds[kind] } : {}),
    })),
  ];
}

/** The chosen kind. A query with several kinds shows as „Alle“: the control picks one. */
function typeValue(filters: WorkFilters): string {
  const [kind] = filters.kinds;
  return filters.kinds.length === 1 && kind ? kind : ALL;
}

function setType(filters: WorkFilters, onChange: (next: WorkFilters) => void, id: string) {
  onChange({ ...filters, kinds: id === ALL ? [] : [id as RecordKind] });
}

function Controls({
  filters,
  onChange,
  directory,
  me,
  onReset,
  searchRef,
  field,
}: Omit<WorkToolbarProps, "counts" | "narrow" | "title" | "onCreate"> & { field: DebouncedText }) {
  const props = { filters, onChange };
  return (
    <>
      <SearchBox field={field} inputRef={searchRef} />
      <OwnerFilter {...props} directory={directory} me={me} />
      <StreamFilter {...props} directory={directory} />
      <DueFilter {...props} />
      <ResetFilters filters={filters} onReset={onReset} />
    </>
  );
}

/**
 * The „Filter“ button and the sheet behind it, which holds the search and the other filters
 * (ADR 0082). The button says how many of them are set; the type has its own control.
 */
function FilterSheet({
  compact,
  ...props
}: Omit<WorkToolbarProps, "counts" | "narrow" | "title" | "onCreate"> & {
  field: DebouncedText;
  /** The ghost button of the toolbar row; a phone has the plain button. */
  compact: boolean;
}) {
  const [open, setOpen] = useState(false);
  const active = countOtherFilters(props.filters);
  return (
    <>
      <Button
        variant={compact ? "quiet" : "secondary"}
        size={compact ? "sm" : "md"}
        onPress={() => setOpen(true)}
        {...(active > 0 ? { "aria-label": t("work-filter-open-active", { count: active }) } : {})}
      >
        <IconFilter size={14} stroke={1.5} aria-hidden="true" />
        {t("work-filter-open")}
        {active > 0 && <span className={styles.badge}>{active}</span>}
      </Button>
      {open && (
        <Sheet title={t("work-filter-title")} onClose={() => setOpen(false)}>
          <div className={styles.sheet}>
            <Controls {...props} />
          </div>
        </Sheet>
      )}
    </>
  );
}

/** What the view tabs place around their own list: the toolbar row is one row (ADR 0082). */
export interface WorkToolbarSlots {
  before: ReactNode;
  after?: ReactNode;
}

/**
 * The toolbar of the „Arbeit“ page as the two slots of its row, which the view tabs complete: the
 * title and the type filter, then the filter button, then the tabs. On a phone the type is a
 * select, and it sits with the button in a second row below the title and the tabs. It is a hook
 * because the pending text of the search must outlive a sheet that closes.
 */
export function useWorkToolbar({ narrow, title, ...props }: WorkToolbarProps): WorkToolbarSlots {
  const field = useSearchText(props);
  const { filters, onChange, counts, directory, me, onReset, searchRef, onCreate } = props;
  const sheet = (compact: boolean) => (
    <FilterSheet
      compact={compact}
      filters={filters}
      onChange={onChange}
      directory={directory}
      me={me}
      onReset={onReset}
      searchRef={searchRef}
      field={field}
    />
  );
  if (narrow) {
    return {
      before: <div className={styles.lead}>{title}</div>,
      after: (
        <div className={styles.narrow}>
          <Select
            label={t("work-type-label")}
            labelHidden
            variant="toolbar"
            options={typeOptions(counts).map(({ id, label, count }) => ({
              id,
              label: count === undefined ? label : `${label} (${count})`,
            }))}
            value={typeValue(filters)}
            onChange={(id) => setType(filters, onChange, id)}
          />
          {sheet(false)}
          {onCreate && <NewButton onCreate={onCreate} />}
        </div>
      ),
    };
  }
  return {
    before: (
      <>
        <div className={styles.lead}>
          {title}
          <TypeFilter
            label={t("work-type-label")}
            options={typeOptions(counts)}
            value={typeValue(filters)}
            onChange={(id) => setType(filters, onChange, id)}
          />
        </div>
        <div className={styles.filter}>{sheet(true)}</div>
      </>
    ),
    after: onCreate ? <NewButton onCreate={onCreate} /> : undefined,
  };
}
