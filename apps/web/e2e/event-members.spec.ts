import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";
import {
  events,
  fakeEvent,
  fakeMemberships,
  fakeSession,
  fontsLoaded,
  forbidden,
  eventMemberships as memberships,
  sessionWithRole,
  setTheme,
  textOverflows,
  themes,
  viewports,
} from "./fixtures";

const event = events[0];

async function fakeMembers(page: Page, memberships_: unknown, status = 200): Promise<void> {
  await fakeEvent(page, event);
  await fakeMemberships(page, memberships_, status);
}

const states = [
  { name: "manager", role: "member", status: 200, items: memberships },
  { name: "forbidden", role: "member", status: 403, items: forbidden },
] as const;

for (const viewport of viewports) {
  for (const theme of themes) {
    for (const state of states) {
      test(`event members, ${state.name}, ${theme}, ${viewport.name} px: no axe violation${
        state.name === "manager" ? ", screenshot" : ""
      }`, async ({ page }) => {
        await page.setViewportSize(viewport);
        await fakeSession(page, sessionWithRole(state.role));
        await fakeMembers(page, state.items, state.status);
        await page.goto(`/events/${event?.id}/members`);
        await setTheme(page, theme);
        await expect(page.getByRole("heading", { level: 1, name: event?.name })).toBeVisible();
        if (state.status === 200) {
          await expect(page.getByRole("table")).toBeVisible();
        } else {
          // A member who may not list the members sees no section and no error.
          await page.waitForLoadState("networkidle");
          await expect(page.getByRole("table")).toHaveCount(0);
          await expect(page.getByRole("main").getByRole("alert")).toHaveCount(0);
        }
        if (state.name === "manager") {
          await expect(page.getByRole("button", { name: "Mitglied hinzufügen" })).toBeVisible();
        }

        const results = await new AxeBuilder({ page })
          .withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"])
          .analyze();
        expect(results.violations).toEqual([]);

        if (state.name === "manager") {
          await fontsLoaded(page);
          await expect(page).toHaveScreenshot(`event-members-${theme}-${viewport.name}.png`, {
            fullPage: true,
          });
        }
      });
    }
  }
}

test("an event manager picks the member to add from the organization", async ({ page }) => {
  await fakeSession(page, sessionWithRole("member"));
  await fakeMembers(page, memberships);
  await page.goto(`/events/${event?.id}/members`);

  await page.getByRole("button", { name: /Mitglied wählen/ }).click();
  await page.getByRole("option", { name: "Cäcilia Probst" }).click();
  await expect(page.getByRole("button", { name: /Cäcilia Probst/ })).toBeVisible();
});

// ADR 0024: no text overflows with the 40 % longer pseudo-locale.
for (const viewport of viewports) {
  test(`event members, pseudo-locale, ${viewport.name} px: no text overflows`, async ({ page }) => {
    await page.setViewportSize(viewport);
    await fakeSession(page, sessionWithRole("member"));
    await fakeMembers(page, memberships);
    await page.goto(`/events/${event?.id}/members?pseudo`);
    await expect(page.getByRole("table")).toBeVisible();
    await expect(page.locator("[role=status][aria-label]")).toHaveCount(0);

    expect(await textOverflows(page)).toEqual([]);
  });
}
