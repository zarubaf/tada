import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";
import { fontsLoaded, setTheme, themes, viewports } from "./fixtures";
import { event, fakeWork } from "./workFixtures";

// „Bereiche verwalten“ on the „Arbeit“ page (ADR 0082). The unit tests hold the rules of the
// dialog; these checks hold what only a browser shows: the sheet over the page in both themes and
// widths, the way in from the filter, and a workstream created end to end.
const work = `/events/${event?.id}/work`;
const MADE = "0199b8e0-0000-7000-8000-0000000000c9";

const axe = (page: Page) =>
  new AxeBuilder({ page }).withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"]).analyze();

async function open(page: Page) {
  await fakeWork(page);
  await page.clock.setFixedTime(new Date("2030-05-08T10:00:00Z"));
  await page.goto(work);
  await expect(page.getByRole("heading", { level: 1, name: "Arbeit" })).toBeVisible();
}

async function openFromFilter(page: Page) {
  await page.getByRole("button", { name: "Filter" }).click();
  await page.getByRole("button", { name: /^Bereich:/ }).click();
  await page.getByRole("button", { name: "Bereiche verwalten …" }).click();
  return page.getByRole("dialog", { name: "Bereiche verwalten" });
}

for (const viewport of viewports) {
  for (const theme of themes) {
    const name = `${theme}, ${viewport.name} px`;

    test(`the dialog opens from the filter, ${name}: no axe violation, screenshot`, async ({
      page,
    }) => {
      await page.setViewportSize(viewport);
      await open(page);
      await setTheme(page, theme);
      const dialog = await openFromFilter(page);
      await expect(
        dialog.getByRole("list", { name: "Bereiche" }).getByRole("listitem"),
      ).toHaveCount(2);
      await expect(dialog.getByRole("textbox", { name: /^Name/ })).toBeVisible();
      expect((await axe(page)).violations).toEqual([]);
      await fontsLoaded(page);
      await expect(page).toHaveScreenshot(`workstreams-${theme}-${viewport.name}.png`);
    });

    test(`a workstream is made in the dialog, ${name}: no axe violation`, async ({ page }) => {
      await page.setViewportSize(viewport);
      await open(page);
      await setTheme(page, theme);
      const made = {
        id: MADE,
        event_id: event?.id,
        name: "Gelände und Parkplätze",
        lead_user_id: "0199b8e0-0000-7000-8000-0000000000b1",
        status: "active",
        version: 1,
      };
      let rows: unknown[] | undefined;
      let body: Record<string, unknown> | undefined;
      await page.route("**/api/v1/events/*/workstreams", async (route) => {
        if (route.request().method() === "POST") {
          body = route.request().postDataJSON();
          rows = [made];
          return route.fulfill({
            status: 201,
            contentType: "application/json",
            body: JSON.stringify(made),
          });
        }
        return rows
          ? route.fulfill({
              status: 200,
              contentType: "application/json",
              body: JSON.stringify({ items: rows }),
            })
          : route.fallback();
      });

      const dialog = await openFromFilter(page);
      await dialog.getByRole("textbox", { name: /^Name/ }).fill("Gelände und Parkplätze");
      await dialog.getByRole("button", { name: "Erfassen" }).click();

      await expect(
        dialog.getByRole("list", { name: "Bereiche" }).getByText("Gelände und Parkplätze"),
      ).toBeVisible();
      expect(body).toMatchObject({ name: "Gelände und Parkplätze" });
      expect((await axe(page)).violations).toEqual([]);

      await page.keyboard.press("Escape");
      await expect(dialog).toBeHidden();
      await expect(page.getByRole("heading", { level: 1, name: "Arbeit" })).toBeFocused();
    });
  }
}
