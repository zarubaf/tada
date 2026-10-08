import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";
import {
  events,
  fakeEvent,
  fakeSession,
  fontsLoaded,
  sessionInfo,
  sessionWithRole,
  setTheme,
  themes,
  viewports,
} from "./fixtures";

const event = events[0];

// Invented members with long German names and umlauts (doc/design/principles.md).
const memberships = [
  {
    user_id: sessionInfo.user_id,
    display_name: "Anna Muster",
    event_role: "event-manager",
    version: 1,
    created_at: "2028-03-01T13:12:00Z",
  },
  {
    user_id: "0199b8e0-0000-7000-8000-0000000000b2",
    display_name: "Bernhard Beispiel-Schmidlin-Äbischer",
    event_role: "event-contributor",
    version: 2,
    created_at: "2028-03-02T09:00:00Z",
  },
];

const organizationMembers = [
  ...memberships.map(({ user_id, display_name }) => ({
    user_id,
    display_name,
    role: "member",
    version: 1,
  })),
  {
    user_id: "0199b8e0-0000-7000-8000-0000000000b3",
    display_name: "Cäcilia Probst",
    role: "member",
    version: 1,
  },
];

const forbidden = {
  type: "https://github.com/zarubaf/tada/blob/main/doc/problems.md#forbidden",
  code: "forbidden",
  title: "The caller may not do this.",
  status: 403,
  instance: "urn:uuid:01a1118e-3359-73dd-a500-feed65806a9d",
  request_id: "01a1118e-3359-73dd-a500-feed65806a9d",
};

async function fakeMembers(page: Page, memberships_: unknown, status = 200): Promise<void> {
  await fakeEvent(page, event);
  await page.route("**/api/v1/events/*/memberships", (route) =>
    route.fulfill({
      status,
      contentType: status < 400 ? "application/json" : "application/problem+json",
      body: JSON.stringify(status < 400 ? { items: memberships_ } : memberships_),
    }),
  );
  await page.route("**/api/v1/members*", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ items: organizationMembers }),
    }),
  );
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
          await expect(page.getByRole("main").getByRole("alert")).toBeVisible();
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
