import { IconCheck, IconChevronDown } from "@tabler/icons-react";
import { type ReactNode, useRef, useState } from "react";
import {
  Autocomplete,
  Dialog,
  DialogTrigger,
  Input,
  ListBox,
  ListBoxItem,
  Popover,
  SearchField,
  useFilter,
} from "react-aria-components";
import { t } from "../i18n";
import { Button } from "./Button";
import { useFocusAfterCommit } from "./focus";
import popover from "./Popover.module.css";
import styles from "./SearchSelect.module.css";

export interface SearchOption {
  id: string;
  label: string;
  /** A node before the label, for example an avatar. It is decoration: the label names the option. */
  leading?: ReactNode;
}

export interface SearchSelectProps {
  /** The accessible name of the select and of its list, for example „Zuständig“. */
  label: string;
  /** All options. The component filters them by the typed text. */
  options: SearchOption[];
  /** The ID of the chosen option, or nothing. */
  value: string | undefined;
  onChange: (id: string | undefined) => void;
  /** The text of the trigger while nothing is chosen. */
  placeholder: string;
  /** The text of the button that clears the choice. Without it the choice cannot be cleared. */
  clearLabel?: string;
}

/**
 * A select with a search field in its popover, for a list that is too long to scan: owner,
 * workstream, milestone (doc/design/components.md). `ComboBox` is the other choice: it keeps the
 * input in the page and lets the caller load the options. Here the options are all present.
 */
export function SearchSelect({
  label,
  options,
  value,
  onChange,
  placeholder,
  clearLabel,
}: SearchSelectProps) {
  const [isOpen, setOpen] = useState(false);
  const { contains } = useFilter({ sensitivity: "base" });
  const trigger = useRef<HTMLButtonElement>(null);
  const focusAfterCommit = useFocusAfterCommit();
  // The popover restores focus in a later frame, and not at all when the chosen element is gone.
  const chosen = options.find((option) => option.id === value);

  return (
    <DialogTrigger isOpen={isOpen} onOpenChange={setOpen}>
      <Button ref={trigger} aria-label={`${label}: ${chosen?.label ?? placeholder}`}>
        {chosen?.leading}
        <span className={styles.value}>{chosen?.label ?? placeholder}</span>
        <IconChevronDown size={14} aria-hidden="true" />
      </Button>
      <Popover className={`${popover.popover} ${styles.popover}`} placement="bottom start">
        <Dialog className={styles.dialog} aria-label={label}>
          <Autocomplete filter={contains}>
            <SearchField className={styles.search} aria-label={t("search-select-search")} autoFocus>
              <Input className={styles.input} />
            </SearchField>
            <ListBox
              className={popover.list}
              aria-label={label}
              items={options}
              selectionMode="single"
              disallowEmptySelection
              selectedKeys={value === undefined ? [] : [value]}
              onSelectionChange={(keys) => {
                const [key] = keys === "all" ? [] : [...keys];
                if (key !== undefined) {
                  onChange(String(key));
                  setOpen(false);
                  focusAfterCommit(() => trigger.current);
                }
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
          </Autocomplete>
          {clearLabel && value !== undefined && (
            <Button
              variant="quiet"
              onPress={() => {
                onChange(undefined);
                setOpen(false);
                focusAfterCommit(() => trigger.current);
              }}
            >
              {clearLabel}
            </Button>
          )}
        </Dialog>
      </Popover>
    </DialogTrigger>
  );
}
