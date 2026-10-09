import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";
import {
  fakeSession,
  fontsLoaded,
  sessionWithRole,
  setTheme,
  textOverflows,
  themes,
  viewports,
} from "./fixtures";

// Invented records with long German names and umlauts (doc/design/principles.md).
const persons = [
  {
    id: "0199b8e0-0000-7000-8000-0000000000d1",
    local_id: "PER-001",
    name: "Bernhard Beispiel-Schmidlin-Äbischer",
    email: "bernhard.beispiel-schmidlin@example.org",
    phone: "+41 00 000 00 01",
    version: 1,
  },
  {
    id: "0199b8e0-0000-7000-8000-0000000000d2",
    local_id: "PER-002",
    name: "Cäcilia Probst",
    version: 3,
  },
];

const institutions = [
  {
    id: "0199b8e0-0000-7000-8000-0000000000e1",
    local_id: "INS-001",
    name: "Generatorenvermietung Testwil-Oberdorf Aktiengesellschaft",
    kind: "company",
    email: "info@example.org",
    version: 2,
  },
  {
    id: "0199b8e0-0000-7000-8000-0000000000e2",
    local_id: "INS-002",
    name: "Gemeinde Testwil",
    kind: "authority",
    phone: "+41 00 000 00 02",
    version: 1,
  },
];

async function fakeParties(page: Page): Promise<void> {
  for (const [path, items] of [
    ["persons", persons],
    ["institutions", institutions],
  ] as const) {
    await page.route(`**/api/v1/${path}*`, (route) =>
      route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ items }),
      }),
    );
  }
}

const pages = [
  { name: "persons", path: "/persons", title: "Personen" },
  { name: "institutions", path: "/institutions", title: "Institutionen" },
] as const;

const roles = ["admin", "member"] as const;

for (const viewport of viewports) {
  for (const theme of themes) {
    for (const shown of pages) {
      for (const role of roles) {
        test(`${shown.name}, ${role}, ${theme}, ${viewport.name} px: no axe violation, screenshot`, async ({
          page,
        }) => {
          await page.setViewportSize(viewport);
          await fakeSession(page, sessionWithRole(role));
          await fakeParties(page);
          await page.goto(shown.path);
          await setTheme(page, theme);
          await expect(page.getByRole("heading", { level: 1, name: shown.title })).toBeVisible();
          await expect(page.getByRole("table", { name: shown.title })).toBeVisible();
          await expect(page.getByRole("button", { name: "Erfassen" })).toBeVisible();
          await expect(page.getByRole("button", { name: /bearbeiten$/ })).toHaveCount(
            role === "admin" ? 2 : 0,
          );

          const results = await new AxeBuilder({ page })
            .withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"])
            .analyze();
          expect(results.violations).toEqual([]);

          await fontsLoaded(page);
          await expect(page).toHaveScreenshot(
            `${shown.name}-${role}-${theme}-${viewport.name}.png`,
            { fullPage: true },
          );
        });
      }
    }
  }
}

test("at 320 px the register does not scroll sideways", async ({ page }) => {
  await page.setViewportSize({ width: 320, height: 800 });
  await fakeSession(page, sessionWithRole("admin"));
  await fakeParties(page);
  await page.goto("/institutions");
  await expect(page.getByRole("table", { name: "Institutionen" })).toBeVisible();

  expect(await textOverflows(page)).toEqual([]);
});
