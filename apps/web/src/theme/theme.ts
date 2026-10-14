// The color theme of the member (ADR 0018, ADR 0082): system, light or dark.
// "system" is the default and stores nothing. `public/theme-init.js` applies a stored choice before
// the first paint with the same key and the same rule; keep both in step.

export const THEMES = ["system", "light", "dark"] as const;
export type Theme = (typeof THEMES)[number];

export const THEME_KEY = "tada-theme";

/** The theme after `theme` in the cycle system, light, dark. */
export function nextTheme(theme: Theme): Theme {
  return THEMES[(THEMES.indexOf(theme) + 1) % THEMES.length] as Theme;
}

/** The stored choice. The page works without storage, so a failure means the system. */
export function readTheme(): Theme {
  try {
    const stored = window.localStorage.getItem(THEME_KEY);
    return stored === "light" || stored === "dark" ? stored : "system";
  } catch {
    return "system";
  }
}

/** Stores the choice. The system is the absence of a choice. A failure is not an error. */
export function storeTheme(theme: Theme): void {
  try {
    if (theme === "system") {
      window.localStorage.removeItem(THEME_KEY);
    } else {
      window.localStorage.setItem(THEME_KEY, theme);
    }
  } catch {
    // Private window or blocked storage: the choice lasts until the page closes.
  }
}

/**
 * Sets `data-theme` on the root element. Without the attribute, `prefers-color-scheme` decides in
 * the CSS, so the system needs no listener.
 */
export function applyTheme(theme: Theme): void {
  const root = document.documentElement;
  if (theme === "system") {
    root.removeAttribute("data-theme");
  } else {
    root.setAttribute("data-theme", theme);
  }
}
