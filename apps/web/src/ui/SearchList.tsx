import { IconCheck } from "@tabler/icons-react";
import type { ReactNode } from "react";
import {
  Autocomplete,
  Input,
  ListBox,
  ListBoxItem,
  SearchField,
  useFilter,
} from "react-aria-components";
import { t } from "../i18n";
import popover from "./Popover.module.css";
import styles from "./SearchSelect.module.css";

export interface SearchOption {
  id: string;
  label: string;
  /** A node before the label, for example an avatar. It is decoration: the label names the option. */
  leading?: ReactNode;
}

export interface SearchListProps {
  /** The accessible name of the list, for example „Zuständig“. */
  label: string;
  /** All options. The component filters them by the typed text. */
  options: SearchOption[];
  /** The ID of the chosen option, or nothing. */
  value: string | undefined;
  /** Called with the pressed option; pressing the chosen option again gives `undefined`. */
  onChoose: (id: string | undefined) => void;
  /** False for a short list, where a search field is noise. The list then takes the focus. */
  searchable?: boolean;
}

/**
 * The search field and the list of options, without a trigger or a container. `SearchSelect` puts
 * them in a popover; the bulk bar puts them in a sheet on a phone.
 */
export function SearchList({
  label,
  options,
  value,
  onChoose,
  searchable = true,
}: SearchListProps) {
  const { contains } = useFilter({ sensitivity: "base" });
  const list = (
    <ListBox
      className={popover.list}
      aria-label={label}
      items={options}
      selectionMode="single"
      selectedKeys={value === undefined ? [] : [value]}
      autoFocus={searchable ? false : "first"}
      onSelectionChange={(keys) => {
        const [key] = keys === "all" ? [] : [...keys];
        onChoose(key === undefined ? undefined : String(key));
      }}
      renderEmptyState={() => <p className={styles.empty}>{t("combobox-empty")}</p>}
    >
      {(option: SearchOption) => (
        <ListBoxItem id={option.id} textValue={option.label} className={popover.item}>
          {({ isSelected }) => (
            <>
              {option.leading}
              <span className={styles.label}>{option.label}</span>
              {isSelected && <IconCheck size={14} aria-hidden="true" />}
            </>
          )}
        </ListBoxItem>
      )}
    </ListBox>
  );
  return searchable ? (
    <Autocomplete filter={contains}>
      <SearchField className={styles.search} aria-label={t("search-select-search")} autoFocus>
        <Input className={styles.input} />
      </SearchField>
      {list}
    </Autocomplete>
  ) : (
    list
  );
}
