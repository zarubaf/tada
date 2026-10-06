import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";
import { fontsLoaded, setTheme, themes } from "./fixtures";

const densities = ["compact", "comfortable"] as const;

for (const theme of themes) {
  for (const density of densities) {
    test(`gallery, ${theme}, ${density}`, async ({ page }) => {
      await page.setViewportSize({ width: 1440, height: 900 });
      await page.goto("/_gallery");
      await setTheme(page, theme);
      await page.evaluate(
        (value) => document.documentElement.setAttribute("data-density", value),
        density,
      );
      await expect(page.getByRole("heading", { name: "Komponenten" })).toBeVisible();

      const results = await new AxeBuilder({ page })
        .withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"])
        .analyze();
      expect(results.violations).toEqual([]);
      await fontsLoaded(page);
      await expect(page).toHaveScreenshot(`gallery-${theme}-${density}.png`, { fullPage: true });
    });
  }
}
