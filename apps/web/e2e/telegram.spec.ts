import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";
import { fakeSession, fontsLoaded, setTheme, textOverflows, themes, viewports } from "./fixtures";

// An invented Telegram account with a long name and umlauts (doc/design/principles.md).
const requests = [
  {
    id: "0199b8e0-0000-7000-8000-0000000000d1",
    telegram_user_id: 4711,
    telegram_name: "Bernhard Beispiel-Schmidlin-Äbischer",
    claimed_at: "2028-03-02T09:00:00Z",
  },
];

async function fakeTelegram(page: Page, items: unknown[]): Promise<void> {
  await page.route("**/api/v1/telegram/link-requests", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ items }),
    }),
  );
  await page.route("**/api/v1/telegram/link-codes", (route) =>
    route.fulfill({
      status: 201,
      contentType: "application/json",
      body: JSON.stringify({ code: "K7M3-QX92", expires_at: "2028-03-02T09:10:00Z" }),
    }),
  );
}

const states = [
  { name: "empty", items: [], withCode: false },
  { name: "request", items: requests, withCode: false },
  { name: "code", items: requests, withCode: true },
] as const;

for (const viewport of viewports) {
  for (const theme of themes) {
    for (const state of states) {
      test(`telegram, ${state.name}, ${theme}, ${viewport.name} px: no axe violation, screenshot`, async ({
        page,
      }) => {
        await page.setViewportSize(viewport);
        await fakeSession(page);
        await fakeTelegram(page, [...state.items]);
        await page.goto("/settings/telegram");
        await setTheme(page, theme);
        await expect(
          page.getByRole("heading", { level: 1, name: "Telegram verknüpfen" }),
        ).toBeVisible();
        if (state.items.length > 0) {
          await expect(page.getByRole("table", { name: "Offene Anfragen" })).toBeVisible();
        } else {
          await expect(page.getByText("Keine offenen Anfragen")).toBeVisible();
        }
        if (state.withCode) {
          await page.getByRole("button", { name: "Code erstellen" }).click();
          await expect(page.getByText("K7M3-QX92")).toBeVisible();
        }

        const results = await new AxeBuilder({ page })
          .withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"])
          .analyze();
        expect(results.violations).toEqual([]);

        await fontsLoaded(page);
        await expect(page).toHaveScreenshot(
          `telegram-${state.name}-${theme}-${viewport.name}.png`,
          { fullPage: true },
        );
      });
    }
  }
}

test("the settings navigation leads from the members to Telegram", async ({ page }) => {
  await fakeSession(page);
  await fakeTelegram(page, []);
  await page.route("**/api/v1/members*", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ items: [] }),
    }),
  );
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto("/settings/members");
  await page
    .getByRole("navigation", { name: "Einstellungen" })
    .getByRole("link", { name: "Telegram" })
    .click();
  await expect(page).toHaveURL(/\/settings\/telegram$/);
  await expect(page.getByRole("heading", { level: 1, name: "Telegram verknüpfen" })).toBeVisible();
});

// ADR 0024: German text can be 40 % longer than the source. No text may overflow its box.
for (const viewport of viewports) {
  test(`telegram, pseudo-locale, ${viewport.name} px: no text overflows`, async ({ page }) => {
    await page.setViewportSize(viewport);
    await fakeSession(page);
    await fakeTelegram(page, requests);
    await page.goto("/settings/telegram?pseudo");
    await expect(page.getByRole("table")).toBeVisible();
    // The pseudo-locale changes the names of the buttons, so the first button in the page and the
    // first button in the table stand for „Code erstellen“ and „Bestätigen“.
    await page.locator("main button").first().click();
    await expect(page.getByText("K7M3-QX92")).toBeVisible();
    await expect(page.getByRole("status")).toHaveCount(1);
    expect(await textOverflows(page)).toEqual([]);

    await page.locator("table button").first().click();
    await expect(page.getByRole("alertdialog")).toBeVisible();
    expect(await textOverflows(page)).toEqual([]);
  });
}

for (const theme of themes) {
  test(`telegram, open confirmation, ${theme}: no axe violation`, async ({ page }) => {
    await fakeSession(page);
    await fakeTelegram(page, requests);
    await page.goto("/settings/telegram");
    await setTheme(page, theme);
    await page.getByRole("button", { name: /Bernhard.* bestätigen/ }).click();
    await expect(page.getByRole("alertdialog")).toBeVisible();

    const results = await new AxeBuilder({ page })
      .withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"])
      .analyze();
    expect(results.violations).toEqual([]);
  });
}
