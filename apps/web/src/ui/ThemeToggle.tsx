import { IconDeviceDesktop, IconMoon, IconSun } from "@tabler/icons-react";
import { useEffect, useState } from "react";
import { t } from "../i18n";
import { chooseTheme, nextTheme, readTheme, THEME_EVENT, type Theme } from "../theme/theme";
import { Button } from "./Button";
import { IconButton } from "./IconButton";
import { LiveRegion } from "./LiveRegion";

const ICONS = { system: IconDeviceDesktop, light: IconSun, dark: IconMoon };

/**
 * Cycles the color theme through system, light and dark (ADR 0082). The visible label names the
 * current choice, so the accessible name is the same text. A live region announces each change.
 * `iconOnly` is for the top bar: the icon shows, the same text is the name and the tooltip.
 */
export function ThemeToggle({ iconOnly = false }: { iconOnly?: boolean }) {
  const [theme, setTheme] = useState<Theme>(readTheme);
  const [announcement, setAnnouncement] = useState("");
  const Icon = ICONS[theme];
  const name = (choice: Theme) => t(`theme-${choice}`);

  // Another control, for example the command palette, can choose the theme too.
  useEffect(() => {
    const follow = (event: Event) => setTheme((event as CustomEvent<Theme>).detail);
    window.addEventListener(THEME_EVENT, follow);
    return () => window.removeEventListener(THEME_EVENT, follow);
  }, []);

  const cycle = () => {
    const next = nextTheme(theme);
    chooseTheme(next);
    // The name of the focused button changes silently in many screen readers.
    setAnnouncement(t("theme-announcement", { name: name(next) }));
  };

  return (
    <>
      {iconOnly ? (
        <IconButton icon={Icon} label={t("theme-toggle", { name: name(theme) })} onPress={cycle} />
      ) : (
        <Button variant="quiet" onPress={cycle}>
          <Icon size={16} aria-hidden="true" />
          {t("theme-toggle", { name: name(theme) })}
        </Button>
      )}
      <LiveRegion kind="status" visuallyHidden>
        {announcement}
      </LiveRegion>
    </>
  );
}
