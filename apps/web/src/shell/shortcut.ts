/** True on a platform where the command key is `⌘`. */
function isApplePlatform(): boolean {
  return typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform);
}

/** The hint in the command box: the key combination of the platform. */
export function paletteHint(): string {
  return isApplePlatform() ? "⌘K" : "Ctrl K";
}

/**
 * True for `⌘K` and `Ctrl+K`, which open the command palette: not with Shift, not when another
 * handler took the key, and not while a dialog is open (a confirmation, a sheet or the drawer).
 */
export function isPaletteKey(event: KeyboardEvent): boolean {
  return (
    (event.metaKey || event.ctrlKey) &&
    !event.altKey &&
    !event.shiftKey &&
    !event.defaultPrevented &&
    event.key.toLowerCase() === "k" &&
    document.querySelector('[role="dialog"], [role="alertdialog"]') === null
  );
}
