import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";
import {
  fakePost,
  fakeReminders,
  fakeSession,
  fontsLoaded,
  problemBody,
  setTheme,
  themes,
  viewports,
} from "./fixtures";

const notLinked = { opted_in_at: null, time_zone: "Europe/Zurich", telegram_linked: false };
const linked = { ...notLinked, telegram_linked: true };

for (const viewport of viewports) {
  for (const theme of themes) {
    test(`account, ${theme}, ${viewport.name} px: no axe violation, screenshot`, async ({
      page,
    }) => {
      await page.setViewportSize(viewport);
      await fakeSession(page);
      await fakeReminders(page, notLinked);
      await page.goto("/settings/account");
      await setTheme(page, theme);
      await expect(page.getByRole("button", { name: "Überall abmelden" })).toBeVisible();
      await expect(page.getByRole("switch")).toBeVisible();

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

test("opt_in_needs_link: without a Telegram link the opt-in is off and leads to the link", async ({
  page,
}) => {
  await fakeSession(page);
  await fakeReminders(page, notLinked);
  await page.goto("/settings/account");

  const toggle = page.getByRole("switch", { name: "Erinnerungen in Telegram erhalten" });
  await expect(toggle).toBeDisabled();
  await expect(page.getByText("Für Erinnerungen braucht tada ein verknüpftes")).toBeVisible();
  await page.getByRole("link", { name: "Telegram verknüpfen" }).click();
  await expect(page).toHaveURL(/\/settings\/telegram$/);
});

test("a linked member opts in, and a lost link answers with the reason", async ({ page }) => {
  await fakeSession(page);
  await fakeReminders(page, linked);
  await fakePost(page, "/api/v1/account/reminders/set", 422, {
    ...problemBody("validation-failed", 422),
    errors: [{ pointer: "/opt_in", code: "telegram-not-linked" }],
  });
  await page.goto("/settings/account");

  const toggle = page.getByRole("switch", { name: "Erinnerungen in Telegram erhalten" });
  await expect(toggle).toBeEnabled();
  await fakeReminders(page, notLinked);
  await page.getByText("Erinnerungen in Telegram erhalten").click();

  await expect(page.getByText(/Ihr Telegram-Konto ist nicht mehr verknüpft/)).toBeVisible();
  await expect(toggle).toBeDisabled();
});
