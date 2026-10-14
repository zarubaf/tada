import { IconFilter } from "@tabler/icons-react";
import { useState } from "react";
import type { RecordKind } from "../api/client";
import { t } from "../i18n";
import { Button } from "../ui/Button";
import { Select } from "../ui/Select";
import { Sheet } from "../ui/Sheet";
import { Toolbar } from "../ui/Toolbar";
import { TypeFilter } from "../ui/TypeFilter";
import type { DirectoryState } from "./directory";
import { RECORD_KINDS } from "./recordStatus";
import {
  countOtherFilters,
  DueFilter,
  OwnerFilter,
  ResetFilters,
  SearchBox,
  StreamFilter,
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
  /** Below 40rem the type is a select and the other filters sit in a sheet (ADR 0023). */
  narrow: boolean;
}

function typeOptions(counts: Counts | undefined) {
  const total = counts && RECORD_KINDS.reduce((sum, kind) => sum + counts.kinds[kind], 0);
  return [
    { id: ALL, label: t("work-type-all"), ...(total === undefined ? {} : { count: total }) },
    ...RECORD_KINDS.map((kind) => ({
      id: kind,
      label: t(`work-type-${kind}`),
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
}: Omit<WorkToolbarProps, "counts" | "narrow">) {
  const props = { filters, onChange };
  return (
    <>
      <SearchBox {...props} />
      <OwnerFilter {...props} directory={directory} me={me} />
      <StreamFilter {...props} directory={directory} />
      <DueFilter {...props} />
      <ResetFilters {...props} />
    </>
  );
}

function NarrowToolbar({
  filters,
  onChange,
  counts,
  directory,
  me,
}: Omit<WorkToolbarProps, "narrow">) {
  const [open, setOpen] = useState(false);
  const active = countOtherFilters(filters);
  return (
    <div className={styles.narrow}>
      <Select
        label={t("work-type-label")}
        labelHidden
        options={typeOptions(counts).map(({ id, label, count }) => ({
          id,
          label: count === undefined ? label : `${label} (${count})`,
        }))}
        value={typeValue(filters)}
        onChange={(id) => setType(filters, onChange, id)}
      />
      <Button
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
            <Controls filters={filters} onChange={onChange} directory={directory} me={me} />
          </div>
        </Sheet>
      )}
    </div>
  );
}

/**
 * The toolbar of the „Arbeit“ page: the type filter, then the search and the filters. The view tabs
 * join it when a second view exists (ADR 0082). On narrow layouts it is a select and a button.
 */
export function WorkToolbar({ narrow, ...props }: WorkToolbarProps) {
  if (narrow) {
    return <NarrowToolbar {...props} />;
  }
  const { filters, onChange, counts, directory, me } = props;
  return (
    <div className={styles.wide}>
      <TypeFilter
        label={t("work-type-label")}
        options={typeOptions(counts)}
        value={typeValue(filters)}
        onChange={(id) => setType(filters, onChange, id)}
      />
      <div className={styles.filters}>
        <SearchBox filters={filters} onChange={onChange} />
        <Toolbar label={t("work-toolbar")}>
          <OwnerFilter filters={filters} onChange={onChange} directory={directory} me={me} />
          <StreamFilter filters={filters} onChange={onChange} directory={directory} />
          <DueFilter filters={filters} onChange={onChange} />
          <ResetFilters filters={filters} onChange={onChange} />
        </Toolbar>
      </div>
    </div>
  );
}
