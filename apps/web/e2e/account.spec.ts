import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";
import { fakeSession, fontsLoaded, setTheme, themes, viewports } from "./fixtures";

for (const viewport of viewports) {
  for (const theme of themes) {
    test(`account, ${theme}, ${viewport.name} px: no axe violation, screenshot`, async ({
      page,
    }) => {
      await page.setViewportSize(viewport);
      await fakeSession(page);
      await page.goto("/settings/account");
      await setTheme(page, theme);
      await expect(page.getByRole("button", { name: "Überall abmelden" })).toBeVisible();

      const results = await new AxeBuilder({ page })
        .withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"])
        .analyze();
      expect(results.violations).toEqual([]);

      await fontsLoaded(page);
      await expect(page).toHaveScreenshot(`account-${theme}-${viewport.name}.png`, {
        fullPage: true,
      });
    });
  }
}
