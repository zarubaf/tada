// The single-key shortcuts of the Review Inbox (doc/design/components.md, doc/design/accessibility.md).
// They fire only for a plain key, never inside a text control or a dialog, and never with a
// modifier that the browser or a screen reader owns.

export type Shortcut = "j" | "k" | "a" | "e" | "r";

const SHORTCUTS: readonly string[] = ["j", "k", "a", "e", "r"];

/** The parts of a keyboard event that decide, so that a test needs no real event. */
export interface KeyPress {
  key: string;
  target: EventTarget | null;
  ctrlKey: boolean;
  metaKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  repeat: boolean;
}

// A checkbox, a radio button and a button take no text, so the keys keep working there.
const TEXT_INPUT =
  "input:not([type='checkbox'], [type='radio'], [type='button'], [type='submit'], [type='reset'])";
const TEXT_CONTROLS = `${TEXT_INPUT}, textarea, select, [contenteditable]:not([contenteditable='false'])`;
const DIALOGS = "[role='dialog'], [role='alertdialog']";

/** The shortcut that the key press means, or nothing. */
export function shortcutOf(press: KeyPress): Shortcut | undefined {
  if (press.ctrlKey || press.metaKey || press.altKey || press.shiftKey || press.repeat) {
    return undefined;
  }
  if (!SHORTCUTS.includes(press.key)) {
    return undefined;
  }
  const target = press.target instanceof Element ? press.target : null;
  if (target?.closest(TEXT_CONTROLS) || target?.closest(DIALOGS)) {
    return undefined;
  }
  return press.key as Shortcut;
}
