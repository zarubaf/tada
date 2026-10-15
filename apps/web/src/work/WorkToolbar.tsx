import type { ReactNode, RefObject } from "react";
import type { RecordKind } from "../api/client";
import { t } from "../i18n";
import { Select } from "../ui/Select";
import { TypeFilter } from "../ui/TypeFilter";
import { TypeIcon } from "../ui/TypeIcon";
import type { DirectoryState } from "./directory";
import { FilterControl, useFilterOpen } from "./FilterControl";
import { NewMenu } from "./NewMenu";
import { RECORD_KINDS } from "./recordStatus";
import { useSearchText } from "./WorkFilterControls";
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
  searchRef: RefObject<HTMLInputElement | null>;
  /** The kinds that the member may create. With none there is no „Neu“ button. */
  creatable: readonly RecordKind[];
  /** The „Neu“ button that shows, for the focus after the create form closes. */
  newRef: RefObject<HTMLButtonElement | null>;
  /** Opens the form of a new record of the kind. */
  onCreate: (kind: RecordKind) => void;
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

/**
 * What the view tabs place around their own list in the toolbar row (ADR 0082): `lead` before it,
 * `end` right of the switch, and `below` in a row of its own, for a phone.
 */
export interface WorkToolbarSlots {
  lead: ReactNode;
  end?: ReactNode;
  below?: ReactNode;
}

/**
 * The toolbar of the „Arbeit“ page as the two slots of its row, which the view tabs complete: the
 * title and the type filter, then the tabs, then the filter button. On a phone the type is a
 * select, and it sits with the button in a second row below the title and the tabs. It is a hook
 * because the pending text of the search must outlive a sheet that closes.
 */
export function useWorkToolbar({ narrow, title, ...props }: WorkToolbarProps): WorkToolbarSlots {
  const field = useSearchText(props);
  const open = useFilterOpen(props.searchRef);
  const {
    filters,
    onChange,
    counts,
    directory,
    me,
    onReset,
    searchRef,
    creatable,
    newRef,
    onCreate,
  } = props;
  const filter = (
    <FilterControl
      narrow={narrow}
      filters={filters}
      onChange={onChange}
      directory={directory}
      me={me}
      onReset={onReset}
      searchRef={searchRef}
      field={field}
      {...open}
    />
  );
  if (narrow) {
    return {
      lead: <div className={styles.lead}>{title}</div>,
      below: (
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
          {filter}
          <NewMenu ref={newRef} kinds={creatable} narrow onCreate={onCreate} />
        </div>
      ),
    };
  }
  return {
    lead: (
      <div className={styles.lead}>
        {title}
        <TypeFilter
          label={t("work-type-label")}
          options={typeOptions(counts)}
          value={typeValue(filters)}
          onChange={(id) => setType(filters, onChange, id)}
        />
      </div>
    ),
    end: (
      <>
        {filter}
        <NewMenu ref={newRef} kinds={creatable} narrow={false} onCreate={onCreate} />
      </>
    ),
  };
}
