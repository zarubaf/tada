import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";
import {
  events,
  fakeEvent,
  fakeEvents,
  fakeSession,
  fontsLoaded,
  sessionWithRole,
  setTheme,
  themes,
  viewports,
} from "./fixtures";

const event = events[0];
const created = events[1];

for (const viewport of viewports) {
  for (const theme of themes) {
    test(`create event, ${theme}, ${viewport.name} px: no axe violation, screenshot`, async ({
      page,
    }) => {
      await page.setViewportSize(viewport);
      await fakeSession(page, sessionWithRole("admin"));
      await page.goto("/events/new");
      await setTheme(page, theme);
      await expect(page.getByRole("heading", { name: "Anlass erfassen" })).toBeVisible();
      await expect(page.getByLabel("Zeitzone")).toHaveValue("Europe/Zurich");

      // The error state: an empty form shows the messages at the fields.
      await page.getByRole("button", { name: "Anlass erfassen" }).click();
      await expect(
        page.getByText("Das Kürzel hat 2 bis 8 Grossbuchstaben oder Ziffern."),
      ).toBeVisible();

      const results = await new AxeBuilder({ page })
        .withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"])
        .analyze();
      expect(results.violations).toEqual([]);
      await fontsLoaded(page);
      await expect(page).toHaveScreenshot(`event-create-${theme}-${viewport.name}.png`, {
        fullPage: true,
      });
    });

    test(`event page, ${theme}, ${viewport.name} px: no axe violation, screenshot`, async ({
      page,
    }) => {
      await page.setViewportSize(viewport);
      await fakeSession(page, sessionWithRole("admin"));
      await fakeEvent(page, event);
      await page.goto(`/events/${event?.id}`);
      await setTheme(page, theme);
      await expect(page.getByRole("heading", { level: 1, name: event?.name })).toBeVisible();

      const results = await new AxeBuilder({ page })
        .withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"])
        .analyze();
      expect(results.violations).toEqual([]);
      await fontsLoaded(page);
      await expect(page).toHaveScreenshot(`event-page-${theme}-${viewport.name}.png`, {
        fullPage: true,
      });
    });

    test(`events with the create action, ${theme}, ${viewport.name} px: no axe violation`, async ({
      page,
    }) => {
      await page.setViewportSize(viewport);
      await fakeSession(page, sessionWithRole("owner"));
      await fakeEvents(page, 200, { items: events });
      await page.goto("/events");
      await setTheme(page, theme);
      await expect(page.getByRole("link", { name: "Anlass erfassen" })).toBeVisible();

      const results = await new AxeBuilder({ page })
        .withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"])
        .analyze();
      expect(results.violations).toEqual([]);
    });
  }
}

test("a member creates an event and lands on its page", async ({ page }) => {
  await fakeSession(page, sessionWithRole("owner"));
  await fakeEvent(page, created);
  await page.route("**/api/v1/events", (route) =>
    route.fulfill({ status: 201, contentType: "application/json", body: JSON.stringify(created) }),
  );
  await page.goto("/events/new");
  await page.getByLabel("Kürzel").fill("test30");
  await page.getByLabel("Name").fill(created?.name ?? "");
  await page.getByRole("button", { name: "Anlass erfassen" }).click();
  await expect(page).toHaveURL(new RegExp(`/events/${created?.id}$`));
  await expect(page.getByRole("heading", { level: 1, name: created?.name })).toBeVisible();
});
