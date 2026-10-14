import { useEffect } from "react";

/** True where a „/“ is text: a field, or a control that takes typing. */
function takesText(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) {
    return false;
  }
  return (
    target.isContentEditable ||
    target.closest(
      "input, textarea, select, [role='textbox'], [role='searchbox'], [role='combobox']",
    ) !== null
  );
}

/**
 * Calls `onKey` when the member presses „/“, the shortcut to the search of a list
 * (doc/design/components.md). A key that is text in a field, or that carries a modifier, is left
 * alone. The key is taken, so that the browser does not start its quick find.
 */
export function useSlashKey(onKey: () => void): void {
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (
        event.key !== "/" ||
        event.defaultPrevented ||
        event.ctrlKey ||
        event.metaKey ||
        event.altKey ||
        takesText(event.target)
      ) {
        return;
      }
      event.preventDefault();
      onKey();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onKey]);
}
