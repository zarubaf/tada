import { type ReactNode, type RefObject, useRef } from "react";
import { Dialog, DialogTrigger, Popover } from "react-aria-components";
import { useFocusAfterCommit } from "./focus";
import popover from "./Popover.module.css";
import styles from "./PopoverDialog.module.css";
import { Sheet } from "./Sheet";

export interface PopoverDialogProps {
  /** The title of the sheet and the accessible name of the popover. */
  title: string;
  /** A phone gets a sheet; a wide layout gets a popover that leaves the page usable. */
  narrow: boolean;
  isOpen: boolean;
  onOpenChange: (open: boolean) => void;
  /** The button that opens the dialog; the dialog marks it as expanded. */
  trigger: ReactNode;
  /** The element of that button: focus returns to it when Escape closes the popover. */
  triggerRef: RefObject<HTMLElement | null>;
  children: ReactNode;
}

/**
 * A button and the dialog behind it. On a wide layout the dialog is a popover that is not modal,
 * so the page behind it keeps working and shows the result of each change at once, for example
 * the filters of a list. On a phone it is a sheet (ADR 0023). Escape closes it and focus returns
 * to the button.
 */
export function PopoverDialog({
  title,
  narrow,
  isOpen,
  onOpenChange,
  trigger,
  triggerRef,
  children,
}: PopoverDialogProps) {
  const focusAfterCommit = useFocusAfterCommit();
  const dialog = useRef<HTMLElement>(null);
  const change = (open: boolean) => {
    onOpenChange(open);
    if (!open) {
      // A popover that is not modal does not restore focus. Focus goes back to the button if it was
      // inside the popover, which may still be in the page; a member who moved on to another
      // control keeps focus there.
      focusAfterCommit(() => {
        const active = document.activeElement;
        const left = !active || active === document.body || dialog.current?.contains(active);
        return left ? triggerRef.current : null;
      });
    }
  };
  return (
    <DialogTrigger isOpen={isOpen} onOpenChange={change}>
      {trigger}
      {narrow ? (
        isOpen && (
          <Sheet title={title} onClose={() => change(false)}>
            {children}
          </Sheet>
        )
      ) : (
        <Popover
          isNonModal
          placement="bottom end"
          className={`${popover.popover} ${styles.popover}`}
        >
          <Dialog ref={dialog} className={styles.dialog} aria-label={title}>
            {children}
          </Dialog>
        </Popover>
      )}
    </DialogTrigger>
  );
}
