import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";
import {
  events,
  fakeEvent,
  fakeMemberships,
  fakeSession,
  fontsLoaded,
  sessionWithRole,
  setTheme,
  textOverflows,
  themes,
  viewports,
} from "./fixtures";

const closed = {
  ...events[0],
  status: "closed",
  closed_at: "2030-07-01T10:00:00Z",
};

for (const viewport of viewports) {
  for (const theme of themes) {
    test(`closed event, ${theme}, ${viewport.name} px: no axe violation, screenshot`, async ({
      page,
    }) => {
      await page.setViewportSize(viewport);
      await fakeSession(page, sessionWithRole("owner"));
      await fakeEvent(page, closed);
      await fakeMemberships(page);
      await page.goto(`/events/${closed.id}`);
      await setTheme(page, theme);
      await expect(page.getByText("Abgeschlossen am 01.07.2030")).toBeVisible();
      await expect(page.getByRole("button", { name: "Wieder öffnen" })).toBeVisible();

      const results = await new AxeBuilder({ page })
        .withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"])
        .analyze();
      expect(results.violations).toEqual([]);

      await fontsLoaded(page);
      await expect(page).toHaveScreenshot(`closed-event-${theme}-${viewport.name}.png`, {
        fullPage: true,
      });
    });
  }
}

for (const viewport of viewports) {
  test(`pseudo-locale, closed event, ${viewport.name} px: no text overflows`, async ({ page }) => {
    await page.setViewportSize(viewport);
    await fakeSession(page, sessionWithRole("owner"));
    await fakeEvent(page, closed);
    await fakeMemberships(page);
    await page.goto(`/events/${closed.id}?pseudo`);
    await expect(page.getByRole("button", { name: /Wíédér/ })).toBeVisible();

    expect(await textOverflows(page)).toEqual([]);
  });
}
