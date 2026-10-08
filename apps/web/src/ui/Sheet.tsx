import type { ReactNode } from "react";
import { Dialog, Heading, Modal, ModalOverlay } from "react-aria-components";
import { t } from "../i18n";
import { Button } from "./Button";
import styles from "./Sheet.module.css";

export interface SheetProps {
  title: string;
  /** Called by the close button, by Escape and by a press outside the sheet. */
  onClose: () => void;
  children: ReactNode;
}

/**
 * A panel for the details of one record: from the right on medium and wide layouts, from the
 * bottom on narrow layouts (doc/design/components.md). It is open while it is mounted. Focus
 * moves into it and returns to the control that opened it when it closes.
 */
export function Sheet({ title, onClose, children }: SheetProps) {
  return (
    <ModalOverlay
      className={styles.overlay}
      isOpen
      isDismissable
      onOpenChange={(open) => {
        if (!open) {
          onClose();
        }
      }}
    >
      <Modal className={styles.sheet}>
        <Dialog className={styles.dialog}>
          <div className={styles.head}>
            <Heading slot="title" className={styles.title}>
              {title}
            </Heading>
            <Button onPress={onClose}>{t("sheet-close")}</Button>
          </div>
          {children}
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}
