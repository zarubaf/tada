import { IconDeviceDesktop, IconMoon, IconSun } from "@tabler/icons-react";
import { useState } from "react";
import { t } from "../i18n";
import { applyTheme, nextTheme, readTheme, storeTheme, type Theme } from "../theme/theme";
import { Button } from "./Button";

const ICONS = { system: IconDeviceDesktop, light: IconSun, dark: IconMoon };

/**
 * Cycles the color theme through system, light and dark (ADR 0082). The visible label names the
 * current choice, so the accessible name is the same text.
 */
export function ThemeToggle() {
  const [theme, setTheme] = useState<Theme>(readTheme);
  const Icon = ICONS[theme];

  const cycle = () => {
    const next = nextTheme(theme);
    storeTheme(next);
    applyTheme(next);
    setTheme(next);
  };

  return (
    <Button variant="quiet" onPress={cycle}>
      <Icon size={16} aria-hidden="true" />
      {t("theme-toggle", { theme })}
    </Button>
  );
}
