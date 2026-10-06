import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";
import {
  events,
  fakeEvents,
  fontsLoaded,
  setTheme,
  themes,
  unavailable,
  viewports,
} from "./fixtures";

const states = [
  { name: "list", status: 200, body: { items: events } },
  { name: "empty", status: 200, body: { items: [] } },
  { name: "error", status: 503, body: unavailable },
] as const;

for (const viewport of viewports) {
  for (const theme of themes) {
    for (const state of states) {
      test(`events, ${state.name}, ${theme}, ${viewport.name} px: no axe violation`, async ({
        page,
      }) => {
        await page.setViewportSize(viewport);
        await fakeEvents(page, state.status, state.body);
        await page.goto("/");
        await setTheme(page, theme);
        await expect(page.getByRole("heading", { name: "Anlässe" })).toBeVisible();
        await expect(page.getByRole("status")).toHaveCount(0);

        const results = await new AxeBuilder({ page })
          .withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"])
          .analyze();
        expect(results.violations).toEqual([]);
      });
    }

    test(`events, list, ${theme}, ${viewport.name} px: screenshot`, async ({ page }) => {
      await page.setViewportSize(viewport);
      await fakeEvents(page, 200, { items: events });
      await page.goto("/");
      await setTheme(page, theme);
      await expect(page.getByRole("table")).toBeVisible();
      await fontsLoaded(page);
      await expect(page).toHaveScreenshot(`events-${theme}-${viewport.name}.png`, {
        fullPage: true,
      });
    });
  }
}

test("the e2e build has the gallery route", async ({ page }) => {
  await page.goto("/_gallery");
  await expect(page.getByRole("heading", { name: "Komponenten" })).toBeVisible();
});
