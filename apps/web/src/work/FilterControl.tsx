import { IconFilter } from "@tabler/icons-react";
import { type RefObject, useRef, useState } from "react";
import { t } from "../i18n";
import { Button } from "../ui/Button";
import { PopoverDialog } from "../ui/PopoverDialog";
import { useSlashKey } from "../ui/useSlashKey";
import type { DirectoryState } from "./directory";
import styles from "./FilterControl.module.css";
import type { DebouncedText } from "./useDebouncedText";
import {
  countOtherFilters,
  DueFilter,
  OwnerFilter,
  ResetFilters,
  SearchBox,
  StreamFilter,
} from "./WorkFilterControls";
import type { WorkFilters } from "./workFilters";

export interface FilterControlProps {
  filters: WorkFilters;
  onChange: (next: WorkFilters) => void;
  directory: DirectoryState;
  /** The ID of the signed-in member. */
  me: string;
  /** Clears all filters. The page moves focus afterwards, because the reset button leaves. */
  onReset: () => void;
  /** The search field that shows, for the focus after a reset and for „/“. */
  searchRef: RefObject<HTMLInputElement | null>;
  /** The text of the search, which the toolbar owns: it outlives a dialog that closes. */
  field: DebouncedText;
  /** How the dialog is open, if it is. The toolbar owns it, see `useFilterOpen`. */
  opened: Opened | undefined;
  onOpened: (opened: Opened | undefined) => void;
  /** A phone has the plain button and a sheet; a wide layout has the ghost button and a popover. */
  narrow: boolean;
}

/** How the control was opened: a phone does not want the keyboard for a tap on the button. */
export type Opened = "button" | "key";

/**
 * Whether the filter dialog is open, and the key „/“ that opens it with focus in the search. The
 * toolbar calls it and not the control, because the tabs build their children twice (a hidden
 * copy collects the tabs), and the key must have one listener.
 */
export function useFilterOpen(searchRef: RefObject<HTMLInputElement | null>) {
  const [opened, setOpened] = useState<Opened>();
  useSlashKey(() => {
    if (opened === undefined) {
      setOpened("key");
    } else {
      searchRef.current?.focus();
    }
  });
  return { opened, onOpened: setOpened };
}

/**
 * The „Filter“ button and what it opens, which holds the search and the other filters (ADR 0082).
 * The button says how many filters are set and whether it is open. „/“ opens it with focus in the
 * search. The type has its own control.
 */
export function FilterControl({
  narrow,
  searchRef,
  field,
  opened,
  onOpened,
  ...props
}: FilterControlProps) {
  const active = countOtherFilters(props.filters);
  const button = useRef<HTMLButtonElement>(null);

  return (
    <PopoverDialog
      title={t("work-filter-title")}
      narrow={narrow}
      isOpen={opened !== undefined}
      onOpenChange={(open) => onOpened(open ? "button" : undefined)}
      triggerRef={button}
      trigger={
        <Button
          ref={button}
          variant={narrow ? "secondary" : "quiet"}
          size={narrow ? "md" : "sm"}
          aria-haspopup="dialog"
          {...(active > 0 ? { "aria-label": t("work-filter-open-active", { count: active }) } : {})}
        >
          <IconFilter size={14} stroke={1.5} aria-hidden="true" />
          {t("work-filter-open")}
          {active > 0 && <span className={styles.badge}>{active}</span>}
        </Button>
      }
    >
      <div className={styles.controls}>
        <SearchBox field={field} inputRef={searchRef} autoFocus={opened === "key" || !narrow} />
        <OwnerFilter {...props} />
        <StreamFilter {...props} />
        <DueFilter {...props} />
        <ResetFilters filters={props.filters} onReset={props.onReset} />
      </div>
    </PopoverDialog>
  );
}
