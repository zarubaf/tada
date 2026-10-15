import { expect, test } from "@playwright/test";
import { fakeMemberships } from "./fixtures";
import { event, fakeWork } from "./workFixtures";

// The old addresses of the pages of an event stay in digests, Telegram messages and bookmarks
// (ADR 0082): each one leads to its new place and replaces its entry in the history.

const base = `/events/${event?.id}`;

test("an old link to an action opens the action in the panel of the Arbeit page", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await fakeWork(page);
  await page.goto("/");
  await page.goto(`${base}/actions#ACT-002`);

  await expect(page).toHaveURL(new RegExp(`${base}/work\\?type=action#ACT-002$`));
  await expect(page.getByRole("complementary", { name: /^ACT-002/ })).toBeVisible();
  // The old address is not in the history: Back leaves the app's start page, not a redirect loop.
  await page.goBack();
  await expect(page).toHaveURL(/\/$/);
});

test("the old page of the timeline opens the timeline view", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await fakeWork(page);
  await page.goto(`${base}/timeline`);

  await expect(page).toHaveURL(new RegExp(`${base}/work\\?view=timeline$`));
  await expect(page.getByRole("heading", { level: 1, name: "Arbeit" })).toBeVisible();
});

test("the old page of the members opens the section of the overview", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await fakeWork(page);
  await fakeMemberships(page);
  await page.goto(`${base}/members`);

  await expect(page).toHaveURL(new RegExp(`${base}#mitglieder$`));
  await expect(page.getByRole("heading", { name: "Mitglieder des Anlasses" })).toBeFocused();
});
