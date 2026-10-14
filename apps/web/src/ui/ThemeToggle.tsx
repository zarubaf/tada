import { IconDeviceDesktop, IconMoon, IconSun } from "@tabler/icons-react";
import { useState } from "react";
import { t } from "../i18n";
import { applyTheme, nextTheme, readTheme, storeTheme, type Theme } from "../theme/theme";
import { Button } from "./Button";
import { LiveRegion } from "./LiveRegion";

const ICONS = { system: IconDeviceDesktop, light: IconSun, dark: IconMoon };

/**
 * Cycles the color theme through system, light and dark (ADR 0082). The visible label names the
 * current choice, so the accessible name is the same text. A live region announces each change.
 */
export function ThemeToggle() {
  const [theme, setTheme] = useState<Theme>(readTheme);
  const [announcement, setAnnouncement] = useState("");
  const Icon = ICONS[theme];
  const name = (choice: Theme) => t(`theme-${choice}`);

  const cycle = () => {
    const next = nextTheme(theme);
    storeTheme(next);
    applyTheme(next);
    setTheme(next);
    // The name of the focused button changes silently in many screen readers.
    setAnnouncement(t("theme-announcement", { name: name(next) }));
  };

  return (
    <>
      <Button variant="quiet" onPress={cycle}>
        <Icon size={16} aria-hidden="true" />
        {t("theme-toggle", { name: name(theme) })}
      </Button>
      <LiveRegion kind="status" visuallyHidden>
        {announcement}
      </LiveRegion>
    </>
  );
}
