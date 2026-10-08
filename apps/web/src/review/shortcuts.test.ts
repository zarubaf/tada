import { describe, expect, it } from "vitest";
import { shortcutOf } from "./shortcuts";

function press(key: string, target: Element, extra: Partial<KeyboardEvent> = {}) {
  return {
    key,
    target,
    ctrlKey: false,
    metaKey: false,
    altKey: false,
    shiftKey: false,
    repeat: false,
    ...extra,
  };
}

const body = document.createElement("div");

describe("shortcutOf", () => {
  it.each(["j", "k", "a", "e", "r"])("accepts the plain key %s", (key) => {
    expect(shortcutOf(press(key, body))).toBe(key);
  });

  it("ignores other keys", () => {
    expect(shortcutOf(press("x", body))).toBeUndefined();
    expect(shortcutOf(press("Enter", body))).toBeUndefined();
  });

  it.each([{ ctrlKey: true }, { metaKey: true }, { altKey: true }, { shiftKey: true }])(
    "ignores a key with a modifier %o",
    (modifier) => {
      expect(shortcutOf(press("a", body, modifier))).toBeUndefined();
    },
  );

  it("ignores a key that repeats", () => {
    expect(shortcutOf(press("j", body, { repeat: true }))).toBeUndefined();
  });

  it.each(["input", "textarea", "select"])("ignores a key typed in a %s", (tag) => {
    expect(shortcutOf(press("a", document.createElement(tag)))).toBeUndefined();
  });

  it("ignores a key typed in a contenteditable element", () => {
    const editable = document.createElement("div");
    editable.setAttribute("contenteditable", "true");
    expect(shortcutOf(press("a", editable))).toBeUndefined();
  });

  it("ignores a key typed inside a dialog", () => {
    const dialog = document.createElement("div");
    dialog.setAttribute("role", "alertdialog");
    const button = document.createElement("button");
    dialog.append(button);
    expect(shortcutOf(press("r", button))).toBeUndefined();
  });
});
