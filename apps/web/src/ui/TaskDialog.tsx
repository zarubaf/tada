import type { ReactNode } from "react";
import { Dialog, Heading, Modal, ModalOverlay } from "react-aria-components";
import styles from "./ConfirmDialog.module.css";

export interface TaskDialogProps {
  title: string;
  /** Called by Escape. The dialog closes only when the caller unmounts it. */
  onCancel: () => void;
  children: ReactNode;
}

/**
 * A dialog for a blocking task with a field, for example a required reason (doc/design/components.md).
 * It is open while it is mounted. The content brings its own buttons; `ConfirmDialog` is the
 * destructive confirmation.
 */
export function TaskDialog({ title, onCancel, children }: TaskDialogProps) {
  return (
    <ModalOverlay
      className={styles.overlay}
      isOpen
      onOpenChange={(open) => {
        if (!open) {
          onCancel();
        }
      }}
    >
      <Modal className={styles.modal}>
        <Dialog className={styles.dialog}>
          <Heading slot="title" className={styles.title}>
            {title}
          </Heading>
          {children}
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}

/** The row of buttons at the end of a task dialog. */
export function TaskDialogActions({ children }: { children: ReactNode }) {
  return <div className={styles.actions}>{children}</div>;
}
