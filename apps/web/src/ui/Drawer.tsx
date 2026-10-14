import { IconX } from "@tabler/icons-react";
import type { ReactNode } from "react";
import { Dialog, Modal, ModalOverlay } from "react-aria-components";
import { t } from "../i18n";
import styles from "./Drawer.module.css";
import { IconButton } from "./IconButton";

export interface DrawerProps {
  /** The accessible name of the drawer, for example „Navigation“. */
  title: string;
  /** Called by the close button, by Escape and by a press outside the drawer. */
  onClose: () => void;
  children: ReactNode;
}

/**
 * A panel from the left edge for the navigation on narrow layouts (doc/design/components.md). It is
 * open while it is mounted. Focus stays inside, the page behind is inert, and focus returns to the
 * control that opened it when it closes. A page that loads it lazily keeps its first load small.
 */
export default function Drawer({ title, onClose, children }: DrawerProps) {
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
      <Modal className={styles.drawer}>
        <Dialog className={styles.dialog} aria-label={title}>
          <div className={styles.close}>
            <IconButton icon={IconX} label={t("shell-drawer-close")} onPress={onClose} />
          </div>
          {children}
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}
