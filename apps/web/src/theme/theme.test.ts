import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, describe, expect, it, vi } from "vitest";
import { applyTheme, nextTheme, readTheme, storeTheme, THEME_KEY } from "./theme";

const root = document.documentElement;

afterEach(() => {
  vi.restoreAllMocks();
  localStorage.clear();
  root.removeAttribute("data-theme");
});

describe("nextTheme", () => {
  it("cycles system, light, dark and back", () => {
    expect(nextTheme("system")).toBe("light");
    expect(nextTheme("light")).toBe("dark");
    expect(nextTheme("dark")).toBe("system");
  });
});

describe("readTheme", () => {
  it("follows the system without a stored choice", () => {
    expect(readTheme()).toBe("system");
  });

  it("reads a stored choice", () => {
    localStorage.setItem(THEME_KEY, "dark");
    expect(readTheme()).toBe("dark");
  });

  it("ignores a stored value that is not a theme", () => {
    localStorage.setItem(THEME_KEY, "sepia");
    expect(readTheme()).toBe("system");
  });

  it("follows the system when the storage throws", () => {
    vi.spyOn(window, "localStorage", "get").mockImplementation(() => {
      throw new DOMException("blocked", "SecurityError");
    });
    expect(readTheme()).toBe("system");
  });
});

describe("storeTheme", () => {
  it("stores light and dark", () => {
    storeTheme("light");
    expect(localStorage.getItem(THEME_KEY)).toBe("light");
    storeTheme("dark");
    expect(localStorage.getItem(THEME_KEY)).toBe("dark");
  });

  it("removes the choice for the system", () => {
    storeTheme("dark");
    storeTheme("system");
    expect(localStorage.getItem(THEME_KEY)).toBeNull();
  });

  it("does not throw when the storage throws", () => {
    vi.spyOn(window, "localStorage", "get").mockImplementation(() => {
      throw new DOMException("blocked", "SecurityError");
    });
    expect(() => storeTheme("dark")).not.toThrow();
  });
});

describe("applyTheme", () => {
  it("sets data-theme for light and dark", () => {
    applyTheme("dark");
    expect(root).toHaveAttribute("data-theme", "dark");
    applyTheme("light");
    expect(root).toHaveAttribute("data-theme", "light");
  });

  it("removes data-theme for the system", () => {
    applyTheme("dark");
    applyTheme("system");
    expect(root).not.toHaveAttribute("data-theme");
  });
});

describe("theme-init.js", () => {
  const dir = dirname(fileURLToPath(import.meta.url));
  const script = readFileSync(resolve(dir, "../../public/theme-init.js"), "utf8");
  const run = () => new Function(script)();

  it("applies a stored choice before the app starts", () => {
    localStorage.setItem(THEME_KEY, "light");
    run();
    expect(root).toHaveAttribute("data-theme", "light");
  });

  it("leaves the system alone without a stored choice", () => {
    run();
    expect(root).not.toHaveAttribute("data-theme");
  });

  it("ignores a stored value that is not a theme", () => {
    localStorage.setItem(THEME_KEY, "sepia");
    run();
    expect(root).not.toHaveAttribute("data-theme");
  });

  it("does not throw when the storage throws", () => {
    vi.spyOn(window, "localStorage", "get").mockImplementation(() => {
      throw new DOMException("blocked", "SecurityError");
    });
    expect(run).not.toThrow();
  });
});
