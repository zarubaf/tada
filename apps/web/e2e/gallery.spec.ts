import { expect, test } from "@playwright/test";
import { axeBuilder, fontsLoaded, setTheme, themes } from "./fixtures";

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

      const results = await axeBuilder(page).analyze();
      expect(results.violations).toEqual([]);
      await fontsLoaded(page);
      await expect(page).toHaveScreenshot(`gallery-${theme}-${density}.png`, { fullPage: true });
    });
  }
}
