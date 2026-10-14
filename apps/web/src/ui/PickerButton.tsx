import type { Icon } from "@tabler/icons-react";
import { useId, useRef, useState } from "react";
import { Dialog, DialogTrigger, Popover } from "react-aria-components";
import { Button } from "./Button";
import { useFocusAfterCommit } from "./focus";
import styles from "./PickerButton.module.css";
import popover from "./Popover.module.css";
import { SearchList, type SearchOption } from "./SearchList";
import searchSelect from "./SearchSelect.module.css";
import { Sheet } from "./Sheet";

export interface PickerButtonProps {
  /** The text of the button, the name of the list and the title of the sheet. */
  label: string;
  icon: Icon;
  options: SearchOption[];
  /**
   * Why the button is off. The reason is the description of the button, so it needs no tooltip. An
   * off button refuses the press and keeps focus (`aria-disabled`), so that the reason can be read.
   */
  disabledReason: string | undefined;
  /** A short list needs no search field. */
  searchable?: boolean;
  /** `sm` is the compact button of a bar, as in `Button`. */
  size?: "md" | "sm";
  /** A popover on a wide layout, a sheet from the bottom on a phone (ADR 0023). */
  narrow: boolean;
  onChoose: (id: string) => void;
}

/**
 * A button that opens a list of values and hands the chosen one to the caller. Unlike
 * `SearchSelect` it shows no current value: it starts an action, for example a change of many
 * records at once. The list is a popover on a wide layout and a sheet on a phone.
 */
export function PickerButton({
  label,
  icon: Icon,
  options,
  disabledReason,
  searchable,
  size,
  narrow,
  onChoose,
}: PickerButtonProps) {
  const [isOpen, setOpen] = useState(false);
  const trigger = useRef<HTMLButtonElement>(null);
  const reason = useId();
  const focusAfterCommit = useFocusAfterCommit();

  const refused = disabledReason !== undefined;

  const choose = (id: string | undefined) => {
    setOpen(false);
    focusAfterCommit(() => trigger.current);
    if (id !== undefined) {
      onChoose(id);
    }
  };
  const list = (
    <SearchList
      label={label}
      options={options}
      value={undefined}
      onChoose={choose}
      {...(searchable !== undefined && { searchable })}
    />
  );
  const button = (
    <Button
      ref={trigger}
      variant="quiet"
      {...(size && { size })}
      isRefused={refused}
      aria-describedby={disabledReason === undefined ? undefined : reason}
      onPress={narrow ? () => setOpen(true) : undefined}
    >
      <Icon
        size={size === "sm" ? 14 : 16}
        stroke={1.5}
        aria-hidden="true"
        className={styles.icon}
      />
      {label}
    </Button>
  );
  return (
    // The title is a courtesy for the mouse; the description is the reason for everybody else.
    <span className={styles.picker} title={disabledReason}>
      {disabledReason !== undefined && (
        <span id={reason} className={styles.hidden}>
          {disabledReason}
        </span>
      )}
      {narrow ? (
        <>
          {button}
          {isOpen && (
            <Sheet title={label} onClose={() => choose(undefined)}>
              {list}
            </Sheet>
          )}
        </>
      ) : (
        <DialogTrigger
          isOpen={isOpen}
          onOpenChange={(open) => {
            // A button that refuses the press does not open its list.
            if (!(open && refused)) {
              setOpen(open);
            }
          }}
        >
          {button}
          <Popover className={`${popover.popover} ${searchSelect.popover}`} placement="top start">
            <Dialog className={searchSelect.dialog} aria-label={label}>
              {list}
            </Dialog>
          </Popover>
        </DialogTrigger>
      )}
    </span>
  );
}
