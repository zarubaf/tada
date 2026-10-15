import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";
import { fontsLoaded, setTheme, themes, viewports } from "./fixtures";
import { event, fakeWork, risks } from "./workFixtures";

// The „Neu“ menu of the „Arbeit“ page (ADR 0082). The unit tests hold the permission rules; these
// checks hold what only a browser shows: the open list with the icons, the focus that the menu and
// the create form hand on, and a risk made end to end.
const work = `/events/${event?.id}/work`;

const axe = (page: Page) =>
  new AxeBuilder({ page }).withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"]).analyze();

async function open(page: Page) {
  await fakeWork(page);
  await page.clock.setFixedTime(new Date("2030-05-08T10:00:00Z"));
  await page.goto(work);
  await expect(page.getByRole("heading", { level: 1, name: "Arbeit" })).toBeVisible();
}

for (const viewport of viewports) {
  for (const theme of themes) {
    const name = `${theme}, ${viewport.name} px`;

    test(`the menu is open, ${name}: no axe violation, screenshot`, async ({ page }) => {
      await page.setViewportSize(viewport);
      await open(page);
      await setTheme(page, theme);
      await page.getByRole("button", { name: "Neu" }).click();
      const menu = page.getByRole("menu", { name: "Neu" });
      await expect(menu.getByRole("menuitem")).toHaveText([
        "Aufgabe",
        "Zusage",
        "Meilenstein",
        "Entscheid",
        "Risiko",
      ]);
      expect((await axe(page)).violations).toEqual([]);
      await fontsLoaded(page);
      await expect(page).toHaveScreenshot(`new-menu-${theme}-${viewport.name}.png`);
    });

    test(`a risk is made from the menu, ${name}: no axe violation`, async ({ page }) => {
      await page.setViewportSize(viewport);
      await open(page);
      await setTheme(page, theme);
      const created = { ...risks[0], id: "0199b8e0-0000-7000-8000-000000000999" };
      let body: Record<string, unknown> | undefined;
      await page.route("**/api/v1/events/*/risks", async (route) => {
        if (route.request().method() !== "POST") {
          return route.fallback();
        }
        body = route.request().postDataJSON();
        return route.fulfill({
          status: 201,
          contentType: "application/json",
          body: JSON.stringify({
            ...created,
            local_id: "RSK-002",
            title: body?.title,
            impact: body?.impact,
            likelihood: body?.likelihood,
            status: "open",
            next_statuses: [],
          }),
        });
      });

      const neu = page.getByRole("button", { name: "Neu" });
      await neu.focus();
      await page.keyboard.press("Enter");
      await page.getByRole("menuitem", { name: "Risiko" }).click();
      const panel = page.getByRole("complementary", { name: "Neues Risiko" });
      await expect(panel.getByRole("textbox", { name: /^Titel/ })).toBeFocused();
      expect((await axe(page)).violations).toEqual([]);

      await panel.getByRole("textbox", { name: /^Titel/ }).fill("Stromausfall am Festabend");
      await panel.getByRole("button", { name: "Wirkung (Pflichtfeld): Stufe wählen" }).click();
      await page.getByRole("option", { name: "hoch" }).click();
      await panel
        .getByRole("button", { name: "Wahrscheinlichkeit (Pflichtfeld): Stufe wählen" })
        .click();
      await page.getByRole("option", { name: "mittel" }).click();
      await panel.getByRole("button", { name: "Erstellen" }).click();

      await expect(page.getByRole("complementary", { name: /^RSK-002/ })).toBeVisible();
      expect(body).toMatchObject({
        title: "Stromausfall am Festabend",
        impact: "high",
        likelihood: "medium",
      });
      expect(new URL(page.url()).hash).toBe("#RSK-002");
      expect((await axe(page)).violations).toEqual([]);
    });
  }
}

test("Escape in the create form returns focus to the Neu button", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await open(page);
  await page.getByRole("button", { name: "Neu" }).click();
  await page.getByRole("menuitem", { name: "Aufgabe" }).click();
  await expect(page.getByRole("complementary", { name: "Neue Aufgabe" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("complementary")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Neu" })).toBeFocused();
});

test("Escape in the create form returns focus to the Neu button on a phone", async ({ page }) => {
  await page.setViewportSize({ width: 375, height: 800 });
  await open(page);
  await page.getByRole("button", { name: "Neu" }).click();
  await page.getByRole("menuitem", { name: "Aufgabe" }).click();
  await expect(page.getByRole("complementary", { name: "Neue Aufgabe" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("complementary")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Neu" })).toBeFocused();
});

test("Abbrechen in the create form returns focus to the Neu button on a phone", async ({
  page,
}) => {
  await page.setViewportSize({ width: 375, height: 800 });
  await open(page);
  await page.getByRole("button", { name: "Neu" }).click();
  await page.getByRole("menuitem", { name: "Risiko" }).click();
  await page.getByRole("button", { name: "Abbrechen" }).click();
  await expect(page.getByRole("complementary")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Neu" })).toBeFocused();
});

test("Abbrechen leaves no equal history entry, so that Back leaves the page", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await open(page);
  await page.getByRole("button", { name: "Neu" }).click();
  await page.getByRole("menuitem", { name: "Risiko" }).click();
  await page.getByRole("button", { name: "Abbrechen" }).click();
  await expect(page.getByRole("complementary")).toHaveCount(0);

  await page.goBack();
  await expect(page).not.toHaveURL(new RegExp(`${work}`));
});
