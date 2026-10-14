import { IconChevronDown, IconChevronUp, IconX } from "@tabler/icons-react";
import { type KeyboardEvent, type ReactNode, useEffect, useId, useRef } from "react";
import { t } from "../i18n";
import { Button } from "./Button";
import styles from "./SidePanel.module.css";

export interface SidePanelProps {
  /** The title of the record. It names the region. */
  title: string;
  /** Called by the close button and by Escape. The page unmounts the panel. */
  onClose: () => void;
  /** The previous record of the list. Without it the button is absent. */
  onPrevious?: () => void;
  /** The next record of the list. Without it the button is absent. */
  onNext?: () => void;
  /**
   * Where focus goes on close when the element that opened the panel is gone, for example a card
   * that the list removed. Usually the list.
   */
  returnFocusTo?: () => HTMLElement | null | undefined;
  children: ReactNode;
}

/**
 * The details of one record beside the list (ADR 0082). The panel is non-modal at every size, so
 * it never traps focus and the list stays usable: docked as a column from 80rem, an overlay from
 * the right from 40rem, full screen below. CSS places it, so the layout does not flash on load.
 * It is open while it is mounted. Focus moves into it on open and back to the element that
 * opened it on close. Load it lazily, as a page does with a route.
 */
export function SidePanel({
  title,
  onClose,
  onPrevious,
  onNext,
  returnFocusTo,
  children,
}: SidePanelProps) {
  const region = useRef<HTMLElement>(null);
  const titleId = useId();
  // The latest fallback, read when the panel closes, not when it opens.
  const fallback = useRef(returnFocusTo);
  fallback.current = returnFocusTo;

  useEffect(() => {
    const opener = document.activeElement;
    region.current?.focus();
    return () => {
      const target =
        opener instanceof HTMLElement && opener !== document.body && opener.isConnected
          ? opener
          : fallback.current?.();
      target?.focus();
    };
  }, []);

  function closeOnEscape(event: KeyboardEvent) {
    // A popover inside the panel handles its own Escape first.
    if (event.key === "Escape" && !event.defaultPrevented) {
      event.stopPropagation();
      onClose();
    }
  }

  return (
    <aside
      ref={region}
      className={styles.panel}
      aria-labelledby={titleId}
      tabIndex={-1}
      onKeyDown={closeOnEscape}
    >
      <header className={styles.head}>
        <h2 id={titleId} className={styles.title}>
          {title}
        </h2>
        <div className={styles.actions}>
          {onPrevious && (
            <Button variant="quiet" aria-label={t("side-panel-previous")} onPress={onPrevious}>
              <IconChevronUp size={20} aria-hidden="true" />
            </Button>
          )}
          {onNext && (
            <Button variant="quiet" aria-label={t("side-panel-next")} onPress={onNext}>
              <IconChevronDown size={20} aria-hidden="true" />
            </Button>
          )}
          <Button variant="quiet" aria-label={t("side-panel-close")} onPress={onClose}>
            <IconX size={20} aria-hidden="true" />
          </Button>
        </div>
      </header>
      <div className={styles.body}>{children}</div>
    </aside>
  );
}
