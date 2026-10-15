import { expect, type Page, test } from "@playwright/test";
import {
  axeBuilder,
  fakeSession,
  fontsLoaded,
  sessionWithRole,
  setTheme,
  themes,
  viewports,
} from "./fixtures";

// Invented tokens with a long name and umlauts (doc/design/principles.md).
const tokens = [
  {
    id: "0199b8e0-0000-7000-8000-0000000000e1",
    name: "Claude Code auf dem Laptop von Bernhard Beispiel-Äbischer",
    scope: "read",
    expires_at: "2099-01-01T00:00:00Z",
    notice_version: 1,
    created_at: "2028-03-01T09:00:00Z",
    last_used_at: "2028-03-02T09:00:00Z",
    revoked_at: null,
  },
  {
    id: "0199b8e0-0000-7000-8000-0000000000e2",
    name: "Codex",
    scope: "propose",
    expires_at: "2028-04-01T00:00:00Z",
    notice_version: 1,
    created_at: "2028-03-01T09:00:00Z",
    last_used_at: null,
    revoked_at: "2028-03-05T09:00:00Z",
  },
];

async function fakeJson(page: Page, path: string, body: unknown, status = 200): Promise<void> {
  await page.route(`**${path}`, (route) =>
    route.fulfill({ status, contentType: "application/json", body: JSON.stringify(body) }),
  );
}

async function fakeTokens(page: Page, items: unknown[], enabled = true): Promise<void> {
  await fakeJson(page, "/api/v1/tokens", { items });
  await fakeJson(page, "/api/v1/token-notice", { version: 1 });
  await fakeJson(page, "/api/v1/organization/features", {
    items: [{ feature: "mcp-tokens", enabled, version: 2 }],
  });
}

const states = [
  { name: "empty", items: [], enabled: true, withSecret: false },
  { name: "list", items: tokens, enabled: true, withSecret: false },
  { name: "off", items: tokens, enabled: false, withSecret: false },
  { name: "secret", items: tokens, enabled: true, withSecret: true },
] as const;

for (const viewport of viewports) {
  for (const theme of themes) {
    for (const state of states) {
      test(`tokens, ${state.name}, ${theme}, ${viewport.name} px: no axe violation, screenshot`, async ({
        page,
      }) => {
        await page.setViewportSize(viewport);
        await fakeSession(page);
        await fakeTokens(page, [...state.items], state.enabled);
        await page.goto("/settings?tab=tokens");
        await setTheme(page, theme);
        await expect(
          page.getByRole("heading", { level: 2, name: "Zugangsschlüssel" }),
        ).toBeVisible();
        if (state.items.length > 0) {
          await expect(page.getByRole("table", { name: "Ihre Zugangsschlüssel" })).toBeVisible();
        } else {
          await expect(page.getByText("Noch keine Zugangsschlüssel")).toBeVisible();
        }
        if (state.withSecret) {
          await page.route("**/api/v1/tokens", (route) =>
            route.request().method() === "POST"
              ? route.fulfill({
                  status: 201,
                  contentType: "application/json",
                  body: JSON.stringify({
                    token: tokens[0],
                    secret: "tada_pat_EXAMPLE-SECRET-0123456789",
                  }),
                })
              : route.fallback(),
          );
          await page.getByRole("textbox", { name: /Name/ }).fill("Codex");
          await page.getByText("Ich habe den Hinweis gelesen").click();
          await page.getByRole("button", { name: "Zugangsschlüssel erstellen" }).click();
          await expect(page.getByText("Wird nur einmal angezeigt")).toBeVisible();
        }

        const results = await axeBuilder(page).analyze();
        expect(results.violations).toEqual([]);

        await fontsLoaded(page);
        await expect(page).toHaveScreenshot(`tokens-${state.name}-${theme}-${viewport.name}.png`, {
          fullPage: true,
        });
      });
    }
  }
}

for (const viewport of viewports) {
  for (const theme of themes) {
    test(`organization, ${theme}, ${viewport.name} px: no axe violation, screenshot`, async ({
      page,
    }) => {
      await page.setViewportSize(viewport);
      await fakeSession(page, sessionWithRole("owner"));
      await fakeJson(page, "/api/v1/organization/features", {
        items: [{ feature: "mcp-tokens", enabled: true, version: 2 }],
      });
      await fakeJson(page, "/api/v1/organization/privacy-notice", { markdown: null, version: 1 });
      await fakeJson(page, "/api/v1/organization/retention", {
        closed_event_months: 24,
        source_months: null,
        version: 3,
      });
      await page.goto("/settings?tab=organization");
      await setTheme(page, theme);
      await expect(
        page.getByRole("switch", { name: "Zugangsschlüssel für KI-Programme erlauben" }),
      ).toBeChecked();
      await expect(
        page.getByRole("textbox", { name: "Text der Datenschutzerklärung" }),
      ).toBeVisible();
      await expect(
        page.getByRole("textbox", { name: "Abgeschlossene Anlässe (Monate)" }),
      ).toHaveValue("24");

      const results = await axeBuilder(page).analyze();
      expect(results.violations).toEqual([]);

      await fontsLoaded(page);
      await expect(page).toHaveScreenshot(`organization-${theme}-${viewport.name}.png`, {
        fullPage: true,
      });
    });
  }
}

test("the tabs of the settings lead to the token page and the organization page", async ({
  page,
}) => {
  await fakeSession(page);
  await fakeTokens(page, []);
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto("/settings?tab=tokens");
  await page.getByRole("tab", { name: "Organisation" }).click();
  await expect(page).toHaveURL(/\/settings\?tab=organization$/);
  await page.getByRole("tab", { name: "Zugangsschlüssel" }).click();
  await expect(page).toHaveURL(/\/settings\?tab=tokens$/);
});
