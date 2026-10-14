/** True on a platform where the command key is `⌘`. */
export function isApplePlatform(): boolean {
  return typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform);
}

/** The hint in the command box: the key combination of the platform. */
export function paletteHint(): string {
  return isApplePlatform() ? "⌘K" : "Ctrl K";
}

/** True for `⌘K` and `Ctrl+K`, which open the command palette. */
export function isPaletteKey(event: KeyboardEvent): boolean {
  return (event.metaKey || event.ctrlKey) && !event.altKey && event.key.toLowerCase() === "k";
}
