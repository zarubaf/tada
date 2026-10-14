import { afterEach, describe, expect, it } from "vitest";
import { MODELESS } from "../ui/overlay";
import { isPaletteKey } from "./shortcut";

function ctrlK(): KeyboardEvent {
  return new KeyboardEvent("keydown", { key: "k", ctrlKey: true, cancelable: true });
}

function dialog(attributes: Record<string, string> = {}) {
  const element = document.createElement("section");
  element.setAttribute("role", "dialog");
  for (const [name, value] of Object.entries(attributes)) {
    element.setAttribute(name, value);
  }
  document.body.append(element);
}

afterEach(() => {
  document.body.replaceChildren();
});

describe("isPaletteKey", () => {
  it("takes Ctrl+K and ⌘K", () => {
    expect(isPaletteKey(ctrlK())).toBe(true);
    expect(isPaletteKey(new KeyboardEvent("keydown", { key: "K", metaKey: true }))).toBe(true);
  });

  it("leaves a key with Shift or Alt, or without a modifier", () => {
    expect(isPaletteKey(new KeyboardEvent("keydown", { key: "k" }))).toBe(false);
    expect(
      isPaletteKey(new KeyboardEvent("keydown", { key: "k", ctrlKey: true, shiftKey: true })),
    ).toBe(false);
  });

  it("is blocked while a modal dialog is open", () => {
    dialog();
    expect(isPaletteKey(ctrlK())).toBe(false);
  });

  it("is blocked while a confirmation is open", () => {
    const element = document.createElement("div");
    element.setAttribute("role", "alertdialog");
    document.body.append(element);
    expect(isPaletteKey(ctrlK())).toBe(false);
  });

  it("still opens while a popover that leaves the page usable is open", () => {
    dialog({ [MODELESS]: "" });
    expect(isPaletteKey(ctrlK())).toBe(true);
  });
});
