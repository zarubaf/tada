import { IconChevronDown } from "@tabler/icons-react";
import { useRef, useState } from "react";
import { Dialog, DialogTrigger, Popover } from "react-aria-components";
import { Button } from "./Button";
import { useFocusAfterCommit } from "./focus";
import popover from "./Popover.module.css";
import { SearchList, type SearchOption } from "./SearchList";
import styles from "./SearchSelect.module.css";

export type { SearchOption };

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
  const trigger = useRef<HTMLButtonElement>(null);
  const focusAfterCommit = useFocusAfterCommit();
  const chosen = options.find((option) => option.id === value);

  return (
    <DialogTrigger
      isOpen={isOpen}
      onOpenChange={(open) => {
        setOpen(open);
        if (!open) {
          // Escape and an outside press close the popover too.
          focusAfterCommit(() => trigger.current);
        }
      }}
    >
      <Button ref={trigger} aria-label={`${label}: ${chosen?.label ?? placeholder}`}>
        {chosen?.leading}
        <span className={styles.value}>{chosen?.label ?? placeholder}</span>
        <IconChevronDown size={14} aria-hidden="true" />
      </Button>
      <Popover className={`${popover.popover} ${styles.popover}`} placement="bottom start">
        <Dialog className={styles.dialog} aria-label={label}>
          <SearchList
            label={label}
            options={options}
            value={value}
            onChoose={(id) => {
              // Pressing the chosen option again deselects it: the choice stays, the popover closes.
              if (id !== undefined) {
                onChange(id);
              }
              setOpen(false);
              // The popover restores focus in a later frame, and not at all when the chosen element is gone.
              focusAfterCommit(() => trigger.current);
            }}
          />
          {clearLabel && value !== undefined && (
            <Button
              variant="quiet"
              onPress={() => {
                onChange(undefined);
                setOpen(false);
                // The popover restores focus in a later frame, and not at all when the chosen element is gone.
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
