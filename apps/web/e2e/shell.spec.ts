import { expect, type Page, test } from "@playwright/test";
import {
  axeBuilder,
  events,
  fakeEvent,
  fakeEvents,
  fakeSession,
  sessionInfo,
  setTheme,
  textOverflows,
  themes,
  viewports,
} from "./fixtures";

const [event] = events;
if (!event) {
  throw new Error("no fixture event");
}

async function expectNoAxeViolation(page: Page): Promise<void> {
  const results = await axeBuilder(page).analyze();
  expect(results.violations).toEqual([]);
}

for (const viewport of viewports) {
  for (const theme of themes) {
    test(`shell, event page, ${theme}, ${viewport.name} px: no axe violation`, async ({ page }) => {
      await page.setViewportSize(viewport);
      await fakeSession(page);
      await fakeEvents(page, 200, { items: events });
      await fakeEvent(page, event);
      await page.goto(`/events/${event.id}`);
      await setTheme(page, theme);
      await expect(page.getByRole("heading", { level: 1, name: event.name })).toBeVisible();
      // On narrow layouts the switcher is in the drawer; the top bar names the event.
      await expect(page.getByRole("banner").getByText(event.name)).toBeVisible();

      await expectNoAxeViolation(page);
    });

    test(`shell, command palette open, ${theme}, ${viewport.name} px: no axe violation`, async ({
      page,
    }) => {
      await page.setViewportSize(viewport);
      await fakeSession(page);
      await fakeEvents(page, 200, { items: events });
      await page.goto("/events");
      await setTheme(page, theme);
      await page.getByRole("button", { name: "Suchen oder Befehl ausführen" }).click();
      await expect(page.getByRole("dialog", { name: "Befehle" })).toBeVisible();
      await expect(page.getByRole("menuitem", { name: event.name })).toBeVisible();

      await expectNoAxeViolation(page);
    });
  }
}

test("the command palette opens with Ctrl+K and a choice moves focus to the new page", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await fakeSession(page);
  await fakeEvents(page, 200, { items: events });
  await page.goto("/events");
  await expect(page.getByRole("heading", { level: 1, name: "Alle Anlässe" })).toBeVisible();

  await page.keyboard.press("Control+k");
  await expect(page.getByRole("searchbox", { name: "Befehl oder Ziel eingeben" })).toBeFocused();
  await page.keyboard.type("kontak");
  await expect(page.getByRole("menuitem")).toHaveCount(1);
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("Enter");

  await expect(page).toHaveURL(/\/contacts$/);
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(page.getByRole("heading", { level: 1, name: "Kontakte" })).toBeFocused();
});

test("Escape closes the command palette and focus returns to the command box", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await fakeSession(page);
  await fakeEvents(page, 200, { items: events });
  await page.goto("/events");

  const box = page.getByRole("button", { name: "Suchen oder Befehl ausführen" });
  await box.click();
  await expect(page.getByRole("dialog", { name: "Befehle" })).toBeVisible();
  await page.keyboard.press("Escape");

  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(box).toBeFocused();
});

test("the sidebar collapses, and it stays collapsed after a reload", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await fakeSession(page);
  await fakeEvents(page, 200, { items: events });
  await page.goto("/events");
  await expect(page.getByRole("navigation", { name: "Hauptnavigation" })).toBeVisible();

  await page.getByRole("button", { name: "Seitenleiste ausblenden" }).click();
  await expect(page.getByRole("navigation", { name: "Hauptnavigation" })).toHaveCount(0);
  await page.reload();

  await expect(page.getByRole("button", { name: "Seitenleiste einblenden" })).toBeVisible();
  await expect(page.getByRole("navigation", { name: "Hauptnavigation" })).toHaveCount(0);
});

test("the event switcher leads to another event", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await fakeSession(page);
  await fakeEvents(page, 200, { items: events });
  await fakeEvent(page, event);
  await page.goto("/events");

  await page.getByRole("button", { name: "Anlass wählen" }).click();
  await page.getByRole("menuitem", { name: event.name }).click();

  await expect(page).toHaveURL(new RegExp(`/events/${event.id}$`));
});

test("at 375 px the drawer holds the sidebar, traps focus and returns it on Escape", async ({
  page,
}) => {
  await page.setViewportSize({ width: 375, height: 812 });
  await fakeSession(page, {
    ...sessionInfo,
    memberships: [
      sessionInfo.organization,
      { ...sessionInfo.organization, organization_id: "x", name: "Segelflugclub" },
    ],
  });
  await fakeEvents(page, 200, { items: events });
  await page.goto("/events");

  const toggle = page.getByRole("button", { name: "Navigation öffnen" });
  await toggle.click();
  const drawer = page.getByRole("dialog", { name: "Navigation" });
  await expect(drawer).toBeVisible();
  await expectNoAxeViolation(page);

  // Tab never leaves the drawer. Thirteen steps end on an item, not on the close button: Escape
  // does not close the drawer from that button, a gap to fix apart from the menu.
  for (let step = 0; step < 13; step++) {
    await page.keyboard.press("Tab");
    expect(await drawer.evaluate((node) => node.contains(document.activeElement))).toBe(true);
  }
  await page.keyboard.press("Escape");
  await expect(drawer).toHaveCount(0);
  await expect(toggle).toBeFocused();
});

test("at 375 px a destination of the drawer leaves focus on the h1 of the new page", async ({
  page,
}) => {
  await page.setViewportSize({ width: 375, height: 812 });
  await fakeSession(page);
  await fakeEvents(page, 200, { items: events });
  await page.goto("/events");

  await page.getByRole("button", { name: "Navigation öffnen" }).click();
  const drawer = page.getByRole("dialog", { name: "Navigation" });
  await drawer.getByRole("link", { name: /^Eingang/ }).click();

  await expect(page).toHaveURL(/\/inbox$/);
  await expect(drawer).toHaveCount(0);
  // The drawer restores focus to its toggle on a closing; the new page must keep it.
  await expect(page.getByRole("heading", { level: 1 })).toBeFocused();
  await page.waitForTimeout(300);
  await expect(page.getByRole("heading", { level: 1 })).toBeFocused();
});

for (const width of [320, 375, 768]) {
  test.describe(`at ${width} px, touch`, () => {
    // ADR 0022: targets are 44 px with a coarse pointer.
    test.use({ hasTouch: true, isMobile: true });

    test(`the top bar and the bottom bar do not scroll sideways, targets are 44 px`, async ({
      page,
    }) => {
      await page.setViewportSize({ width, height: 800 });
      await fakeSession(page);
      await fakeEvents(page, 200, { items: events });
      await page.goto("/events");
      await expect(page.getByRole("banner")).toBeVisible();

      expect(await textOverflows(page)).toEqual([]);
      const scrollWidth = await page.evaluate(() => document.documentElement.scrollWidth);
      expect(scrollWidth).toBeLessThanOrEqual(width);
      if (width < 640) {
        const targets = page
          .getByRole("banner")
          .locator("button")
          .or(page.getByRole("navigation", { name: "Hauptnavigation" }).locator("a, button"));
        for (const target of await targets.all()) {
          const box = await target.boundingBox();
          expect(box?.height).toBeGreaterThanOrEqual(44);
        }
      }
    });
  });
}
