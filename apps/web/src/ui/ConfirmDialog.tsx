import { Dialog, Heading, Modal, ModalOverlay } from "react-aria-components";
import { Button } from "./Button";
import styles from "./ConfirmDialog.module.css";

export interface ConfirmDialogProps {
  isOpen: boolean;
  title: string;
  text: string;
  /** An extra warning in the danger color, for example for a change that affects the member. */
  warning?: string | undefined;
  confirmLabel: string;
  cancelLabel: string;
  /** The request runs: the confirm button ignores a second press. */
  isPending?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

/**
 * A destructive confirmation: title, one sentence and two buttons (doc/design/components.md).
 * An alert dialog closes only with a button or Escape, not with a click outside.
 */
export function ConfirmDialog({
  isOpen,
  title,
  text,
  warning,
  confirmLabel,
  cancelLabel,
  isPending,
  onConfirm,
  onCancel,
}: ConfirmDialogProps) {
  return (
    <ModalOverlay
      className={styles.overlay}
      isOpen={isOpen}
      onOpenChange={(open) => {
        if (!open) {
          onCancel();
        }
      }}
    >
      <Modal className={styles.modal}>
        <Dialog className={styles.dialog} role="alertdialog">
          <Heading slot="title" className={styles.title}>
            {title}
          </Heading>
          <p>{text}</p>
          {warning && <p className={styles.warning}>{warning}</p>}
          <div className={styles.actions}>
            <Button onPress={onCancel}>{cancelLabel}</Button>
            <Button variant="primary" isPending={isPending} onPress={onConfirm}>
              {confirmLabel}
            </Button>
          </div>
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}
