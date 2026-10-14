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
  /** False for a short list, where a search field is noise. */
  searchable?: boolean;
  /** `quiet` has no border until hover, for a value in a list of properties. */
  variant?: "secondary" | "quiet";
  /**
   * A change is being saved. The trigger refuses the press but stays focusable, so that a pick
   * cannot be lost and focus does not drop.
   */
  isPending?: boolean;
  /**
   * The ID of the element that shows the error of this select. The trigger points at it, so that
   * assistive technology reads the error with the name. (`aria-invalid` is not allowed on a
   * button, and React Aria drops it.) A required select says so in its `label`, as a required
   * text field does.
   */
  errorId?: string | undefined;
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
  searchable = true,
  variant = "secondary",
  isPending = false,
  errorId,
}: SearchSelectProps) {
  const [isOpen, setOpen] = useState(false);
  const trigger = useRef<HTMLButtonElement>(null);
  const focusAfterCommit = useFocusAfterCommit();
  const chosen = options.find((option) => option.id === value);

  return (
    <DialogTrigger
      isOpen={isOpen}
      onOpenChange={(open) => {
        if (open && isPending) {
          return;
        }
        setOpen(open);
        if (!open) {
          // Escape and an outside press close the popover too.
          focusAfterCommit(() => trigger.current);
        }
      }}
    >
      <Button
        ref={trigger}
        variant={variant}
        aria-label={`${label}: ${chosen?.label ?? placeholder}`}
        isRefused={isPending}
        {...(errorId && { "aria-describedby": errorId })}
      >
        {chosen?.leading}
        {/* A layout that cuts a long value short still shows it whole on hover. */}
        <span className={styles.value} title={chosen?.label}>
          {chosen?.label ?? placeholder}
        </span>
        <IconChevronDown size={14} aria-hidden="true" />
      </Button>
      <Popover className={`${popover.popover} ${styles.popover}`} placement="bottom start">
        <Dialog className={styles.dialog} aria-label={label}>
          <SearchList
            label={label}
            options={options}
            value={value}
            searchable={searchable}
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
