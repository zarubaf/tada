// What the shell remembers in the browser: the state of the sidebar and the last event. The page
// works without storage, so each failure means „nothing remembered“ (doc/design/layout-and-responsiveness.md).

export const SIDEBAR_KEY = "tada-sidebar";
export const EVENT_KEY = "tada-event";

export function readRemembered(key: string): string | undefined {
  try {
    return window.localStorage.getItem(key) ?? undefined;
  } catch {
    return undefined;
  }
}

/** Stores the value; `undefined` forgets it. A failure is not an error. */
export function writeRemembered(key: string, value: string | undefined): void {
  try {
    if (value === undefined) {
      window.localStorage.removeItem(key);
    } else {
      window.localStorage.setItem(key, value);
    }
  } catch {
    // Private window or blocked storage: the choice lasts until the page closes.
  }
}
