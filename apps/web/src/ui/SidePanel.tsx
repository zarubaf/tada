import { IconChevronDown, IconChevronUp, IconX } from "@tabler/icons-react";
import { type KeyboardEvent, type ReactNode, useId, useLayoutEffect, useRef } from "react";
import { t } from "../i18n";
import { Button } from "./Button";
import { makeOutsideInert } from "./inertOutside";
import styles from "./SidePanel.module.css";

/** `--bp-medium` (CSS custom properties do not work in media queries). */
const NARROW = "(width < 40rem)";

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
 * The details of one record beside the list (ADR 0082). From 40rem the panel is non-modal, so it
 * never traps focus and the list stays usable: docked as a column from 80rem, an overlay from the
 * right below that. Below 40rem it is a full-screen sheet, and everything outside it is inert.
 * CSS places it, so the layout does not flash on load. It is open while it is mounted. Focus moves
 * into it on open and, if it is still inside on close, back to the element that opened it. Load
 * it lazily, as a page does with a route. The parent is a flex row for the docked layout.
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

  // A layout effect, because its cleanup runs before React removes the panel. Then the focus is
  // still inside the panel, and we can tell whether the panel owned it.
  useLayoutEffect(() => {
    const panel = region.current;
    const opener = document.activeElement;
    panel?.focus();
    return () => {
      if (!panel?.contains(document.activeElement)) {
        return;
      }
      const target =
        opener instanceof HTMLElement && opener !== document.body && opener.isConnected
          ? opener
          : fallback.current?.();
      target?.focus();
    };
  }, []);

  // Below the medium breakpoint the panel covers the page, so everything outside it is inert: out
  // of the tab order, out of the reading order and out of the scroll.
  useLayoutEffect(() => {
    const panel = region.current;
    const narrow = window.matchMedia?.(NARROW);
    if (!panel || !narrow) {
      return;
    }
    let release: (() => void) | undefined;
    function apply() {
      release?.();
      release = undefined;
      if (panel && narrow?.matches) {
        release = makeOutsideInert(panel);
      }
    }
    apply();
    narrow.addEventListener("change", apply);
    return () => {
      narrow.removeEventListener("change", apply);
      release?.();
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
