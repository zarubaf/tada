import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";
import {
  e2eBuild,
  events,
  fakeEvents,
  fakeSession,
  fakeSignedOut,
  fakeVersion,
  fontsLoaded,
  setTheme,
  textOverflows,
  themes,
  viewports,
} from "./fixtures";

const footer = `tada ${e2eBuild.version} · ${e2eBuild.commit}`;

async function noAxeViolation(page: Page): Promise<void> {
  const results = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"])
    .analyze();
  expect(results.violations).toEqual([]);
}

/** Opens the list of the components: from the footer on wide layouts, from the member menu on narrow ones. */
async function openComponentList(page: Page, narrow: boolean): Promise<void> {
  if (narrow) {
    await page.getByRole("button", { name: "Anna Muster" }).click();
    await page.getByRole("menuitem", { name: `Versionen (${footer})` }).click();
  } else {
    await page.getByRole("button", { name: footer }).click();
  }
  await expect(page.getByRole("dialog", { name: "Versionen" })).toBeVisible();
}

test("footer_shows_version: the footer shows the build and opens the components", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await fakeSession(page);
  await fakeEvents(page, 200, { items: events });
  await fakeVersion(page);
  await page.goto("/events");

  await openComponentList(page, false);
  const sheet = page.getByRole("dialog", { name: "Versionen" });
  const roles = sheet.getByRole("table", { name: "Laufende Prozesse" });
  await expect(roles.getByRole("row", { name: /worker/ })).toContainText("unbekannt");
  await expect(roles.getByRole("row", { name: /worker/ })).toContainText("Andere Version");
  await expect(roles.getByRole("row", { name: /telegram/ })).not.toContainText("Andere Version");
  await expect(sheet.getByRole("row", { name: /Web-Client/ })).toContainText(
    `${e2eBuild.version} · ${e2eBuild.commit}`,
  );
  await expect(
    page.getByText("Eine neue Version von tada ist installiert.", { exact: true }),
  ).toHaveCount(0);

  // Escape closes the sheet, and focus returns to the footer.
  await page.keyboard.press("Escape");
  await expect(page.getByRole("button", { name: footer })).toBeFocused();
});

test("mismatch_asks_reload: another server build asks to load the page again", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await fakeSession(page);
  await fakeEvents(page, 200, { items: events });
  await fakeVersion(page, { version: "0.0.1", commit: "9f8e7d6" });
  await page.goto("/events");

  await expect(
    page.getByText("Eine neue Version von tada ist installiert.", { exact: true }),
  ).toBeVisible();
  // The prompt appears without an action of the member: the polite region of the shell announces it.
  await expect(
    page.getByRole("status").filter({ hasText: "Eine neue Version von tada ist installiert." }),
  ).toHaveCount(1);
  const reload = page.waitForEvent("load");
  await page.getByRole("button", { name: "Seite neu laden" }).click();
  await reload;
});

test("a client that is not signed in sees no version and asks for none", async ({ page }) => {
  let asked = false;
  await fakeSignedOut(page);
  await page.route("**/api/v1/system/version", (route) => {
    asked = true;
    return route.abort();
  });
  await page.goto("/sign-in");

  await expect(page.getByRole("heading", { level: 1 })).toBeVisible();
  await expect(page.getByText(/tada \d+\.\d+\.\d+/)).toHaveCount(0);
  expect(asked).toBe(false);
});

for (const viewport of viewports) {
  for (const theme of themes) {
    test(`versions, ${theme}, ${viewport.name} px: no axe violation, screenshot`, async ({
      page,
    }) => {
      await page.setViewportSize(viewport);
      await fakeSession(page);
      await fakeEvents(page, 200, { items: events });
      await fakeVersion(page, { version: "0.0.1", commit: "9f8e7d6" });
      await page.goto("/events");
      await setTheme(page, theme);
      await expect(
        page.getByText("Eine neue Version von tada ist installiert.", { exact: true }),
      ).toBeVisible();
      await noAxeViolation(page);
      await fontsLoaded(page);
      await expect(page).toHaveScreenshot(`reload-${theme}-${viewport.name}.png`);

      await openComponentList(page, viewport.width < 640);
      await expect(page.getByRole("table", { name: "Laufende Prozesse" })).toBeVisible();
      await noAxeViolation(page);
      expect(await textOverflows(page)).toEqual([]);
      await expect(page).toHaveScreenshot(`versions-${theme}-${viewport.name}.png`);
    });
  }
}
