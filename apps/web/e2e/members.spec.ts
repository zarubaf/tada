import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";
import {
  fakeSession,
  fontsLoaded,
  sessionInfo,
  sessionWithRole,
  setTheme,
  textOverflows,
  themes,
  viewports,
} from "./fixtures";

// Invented members with long German names and umlauts (doc/design/principles.md).
const members = [
  {
    user_id: sessionInfo.user_id,
    display_name: "Anna Muster",
    email: "anna.muster@example.org",
    role: "owner",
    version: 1,
  },
  {
    user_id: "0199b8e0-0000-7000-8000-0000000000b2",
    display_name: "Bernhard Beispiel-Schmidlin-Äbischer",
    email: "bernhard.beispiel-schmidlin@example.org",
    role: "member",
    version: 2,
  },
];

const invitations = [
  {
    id: "0199b8e0-0000-7000-8000-0000000000c1",
    email: "cäcilia.probst@example.org",
    display_name: "Cäcilia Probst",
    role: "admin",
    created_at: "2028-03-02T09:00:00Z",
  },
];

async function fakeMembers(page: Page, withEmail: boolean): Promise<void> {
  await page.route("**/api/v1/members*", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        items: members.map((m) => ({ ...m, email: withEmail ? m.email : null })),
      }),
    }),
  );
  await page.route("**/api/v1/invitations", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ items: invitations }),
    }),
  );
}

const states = [
  { name: "owner", role: "owner" },
  { name: "member", role: "member" },
] as const;

for (const viewport of viewports) {
  for (const theme of themes) {
    for (const state of states) {
      test(`members, ${state.name}, ${theme}, ${viewport.name} px: no axe violation, screenshot`, async ({
        page,
      }) => {
        await page.setViewportSize(viewport);
        await fakeSession(page, sessionWithRole(state.role));
        await fakeMembers(page, state.role !== "member");
        await page.goto("/settings/members");
        await setTheme(page, theme);
        await expect(page.getByRole("heading", { level: 1, name: "Mitglieder" })).toBeVisible();
        await expect(page.getByRole("table", { name: "Mitglieder" })).toBeVisible();
        if (state.role === "owner") {
          await expect(page.getByRole("table", { name: "Offene Einladungen" })).toBeVisible();
          await expect(page.getByRole("button", { name: "Einladen" })).toBeVisible();
        } else {
          await expect(page.getByRole("button", { name: "Einladen" })).toHaveCount(0);
        }

        const results = await new AxeBuilder({ page })
          .withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"])
          .analyze();
        expect(results.violations).toEqual([]);

        await fontsLoaded(page);
        await expect(page).toHaveScreenshot(`members-${state.name}-${theme}-${viewport.name}.png`, {
          fullPage: true,
        });
      });
    }
  }
}

test("at 320 px the tables do not scroll sideways and each row action is in reach", async ({
  page,
}) => {
  await page.setViewportSize({ width: 320, height: 800 });
  await fakeSession(page, sessionWithRole("owner"));
  await fakeMembers(page, true);
  await page.goto("/settings/members");
  const remove = page.getByRole("button", {
    name: "Bernhard Beispiel-Schmidlin-Äbischer entfernen",
  });
  await expect(remove).toBeVisible();

  const box = await remove.boundingBox();
  expect((box?.x ?? 0) + (box?.width ?? 0)).toBeLessThanOrEqual(320);
  expect(await textOverflows(page)).toEqual([]);
});

test("at 320 px all four items of the bottom bar are inside the viewport", async ({ page }) => {
  await page.setViewportSize({ width: 320, height: 800 });
  await fakeSession(page, sessionWithRole("owner"));
  await fakeMembers(page, true);
  await page.goto("/settings/members");
  const items = page.getByRole("navigation", { name: "Hauptnavigation" }).getByRole("link");
  await expect(items).toHaveCount(4);
  for (const item of await items.all()) {
    const box = await item.boundingBox();
    expect(box?.x).toBeGreaterThanOrEqual(0);
    expect((box?.x ?? 0) + (box?.width ?? 0)).toBeLessThanOrEqual(320);
  }
  expect(await textOverflows(page)).toEqual([]);
});

test("at 375 px a focused element never hides under the bottom bar", async ({ page }) => {
  await page.setViewportSize({ width: 375, height: 640 });
  await fakeSession(page, sessionWithRole("owner"));
  await fakeMembers(page, true);
  await page.goto("/settings/members");
  await expect(page.getByRole("table", { name: "Offene Einladungen" })).toBeVisible();

  for (let step = 0; step < 16; step++) {
    await page.keyboard.press("Tab");
    const hidden = await page.evaluate(() => {
      const focused = document.activeElement;
      const bar = document.querySelector("nav[aria-label='Hauptnavigation']");
      if (!focused || focused === document.body || !bar || bar.contains(focused)) {
        return "";
      }
      // The focus ring is 2 px wide with a 2 px offset.
      const bottom = focused.getBoundingClientRect().bottom + 4;
      const top = bar.getBoundingClientRect().top;
      return bottom > top ? `${focused.textContent}: ${bottom} > ${top}` : "";
    });
    expect(hidden).toBe("");
  }
});

test("the navigation item Einstellungen leads to the members", async ({ page }) => {
  await fakeSession(page, sessionWithRole("owner"));
  await fakeMembers(page, true);
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto("/events");
  await page.getByRole("link", { name: "Einstellungen" }).click();
  await expect(page).toHaveURL(/\/settings\/members$/);
  await expect(page.getByRole("heading", { level: 1, name: "Mitglieder" })).toBeVisible();
});

// ADR 0024: German text can be 40 % longer than the source. No text may overflow its box.
for (const viewport of viewports) {
  for (const role of ["owner", "member"] as const) {
    test(`members, ${role}, pseudo-locale, ${viewport.name} px: no text overflows`, async ({
      page,
    }) => {
      await page.setViewportSize(viewport);
      await fakeSession(page, sessionWithRole(role));
      await fakeMembers(page, role !== "member");
      await page.goto("/settings/members?pseudo");
      await expect(page.getByRole("table").first()).toBeVisible();
      if (role === "owner") {
        await expect(page.getByRole("table")).toHaveCount(2);
      }
      await expect(page.getByRole("status")).toHaveCount(1);

      expect(await textOverflows(page)).toEqual([]);
    });
  }
}

for (const theme of themes) {
  test(`members, open confirmation, ${theme}: no axe violation`, async ({ page }) => {
    await fakeSession(page, sessionWithRole("owner"));
    await fakeMembers(page, true);
    await page.goto("/settings/members");
    await setTheme(page, theme);
    await page.getByRole("button", { name: /Bernhard.* entfernen/ }).click();
    await expect(page.getByRole("alertdialog")).toBeVisible();

    const results = await new AxeBuilder({ page })
      .withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"])
      .analyze();
    expect(results.violations).toEqual([]);
  });
}
