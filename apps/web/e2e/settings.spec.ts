import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";
import {
  fakeReminders,
  fakeSession,
  sessionWithRole,
  setTheme,
  themes,
  viewports,
} from "./fixtures";

const notLinked = { opted_in_at: null, time_zone: "Europe/Zurich", telegram_linked: false };

async function fakeMembers(page: import("@playwright/test").Page): Promise<void> {
  await page.route("**/api/v1/members*", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ items: [] }),
    }),
  );
  await page.route("**/api/v1/invitations", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ items: [] }),
    }),
  );
}

for (const viewport of viewports) {
  for (const theme of themes) {
    test(`settings, ${theme}, ${viewport.name} px: one h1, tab row or select, no axe violation`, async ({
      page,
    }) => {
      await page.setViewportSize(viewport);
      await fakeSession(page, sessionWithRole("owner"));
      await fakeReminders(page, notLinked);
      await fakeMembers(page);
      await page.goto("/settings?tab=members");
      await setTheme(page, theme);
      await expect(page.getByRole("heading", { level: 1, name: "Einstellungen" })).toBeVisible();
      await expect(page.getByRole("heading", { level: 2, name: "Mitglieder" })).toBeVisible();
      await expect(page.getByRole("heading", { level: 1 })).toHaveCount(1);
      await expect(page.getByRole("main")).toHaveCount(1);

      if (viewport.width < 640) {
        await expect(page.getByRole("tablist")).toHaveCount(0);
        await expect(page.getByRole("button", { name: /Mitglieder/ }).first()).toBeVisible();
      } else {
        await expect(page.getByRole("tablist", { name: "Bereich" })).toBeVisible();
        const row = await page.locator("[data-page-toolbar]").boundingBox();
        expect(row?.height).toBe(44);
      }
      // The tab row never scrolls sideways.
      const overflow = await page.evaluate(
        () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
      );
      expect(overflow).toBeLessThanOrEqual(0);

      const results = await new AxeBuilder({ page })
        .withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"])
        .analyze();
      expect(results.violations).toEqual([]);
    });
  }
}

test("an old settings address redirects to its tab and Back does not return to it", async ({
  page,
}) => {
  await fakeSession(page, sessionWithRole("owner"));
  await fakeMembers(page);
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto("/");
  await page.goto("/settings/members");
  await expect(page).toHaveURL(/\/settings\?tab=members$/);
  await expect(page.getByRole("tab", { name: "Mitglieder", selected: true })).toBeVisible();
  await page.goBack();
  await expect(page).toHaveURL(/\/$/);
});

test("a member opens a tab that the role does not give and sees why", async ({ page }) => {
  await fakeSession(page, sessionWithRole("member"));
  await fakeReminders(page, notLinked);
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto("/settings?tab=ai-pm");
  await expect(page.getByText("Sie haben keinen Zugriff auf diesen Bereich.")).toBeVisible();
  await expect(page.getByRole("tab", { name: "AI-PM" })).toHaveCount(0);
  await expect(page.getByRole("tab", { name: "Konto", selected: true })).toBeVisible();
  await expect(page).toHaveURL(/\/settings$/);
});
