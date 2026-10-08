import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";
import {
  fakeEvents,
  fakeSession,
  fontsLoaded,
  sessionWithRole,
  setTheme,
  textOverflows,
  themes,
  viewports,
} from "./fixtures";

// An own text with a table, a link and an image, which the renderer shows as text (ADR 0058).
const ownText = [
  "# Datenschutz der Fliegergruppe Testwil",
  "",
  "Verantwortlich ist der Vorstand. Fragen an [vorstand@example.org](mailto:vorstand@example.org).",
  "",
  "| Kategorie | Aufbewahrung |",
  "| --- | --- |",
  "| Veranstaltungsbewilligungsverfahren | Bis zum Austritt |",
  "",
  "![Lageplan des Flugplatzes](https://example.org/plan.png)",
].join("\n");

async function fakeNotice(page: Page, markdown: string | null): Promise<void> {
  await page.route("**/api/v1/organization/privacy-notice", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ markdown, version: markdown === null ? 1 : 2 }),
    }),
  );
}

const states = [
  { name: "template", markdown: null, heading: "Verantwortlich" },
  { name: "own text", markdown: ownText, heading: "Datenschutz der Fliegergruppe Testwil" },
] as const;

for (const viewport of viewports) {
  for (const theme of themes) {
    for (const state of states) {
      test(`privacy, ${state.name}, ${theme}, ${viewport.name} px: no axe violation, screenshot`, async ({
        page,
      }) => {
        await page.setViewportSize(viewport);
        await fakeSession(page, sessionWithRole("member"));
        await fakeNotice(page, state.markdown);
        await page.goto("/privacy");
        await setTheme(page, theme);
        await expect(page.getByRole("heading", { level: 1, name: "Datenschutz" })).toBeVisible();
        await expect(page.getByRole("heading", { name: state.heading })).toBeVisible();
        // The renderer loads no image and makes no remote request.
        await expect(page.locator("main img")).toHaveCount(0);

        const results = await new AxeBuilder({ page })
          .withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"])
          .analyze();
        expect(results.violations).toEqual([]);

        await fontsLoaded(page);
        await expect(page).toHaveScreenshot(
          `privacy-${state.name.replace(" ", "-")}-${theme}-${viewport.name}.png`,
          { fullPage: true },
        );
      });
    }
  }
}

for (const viewport of viewports) {
  test(`pseudo-locale, privacy, ${viewport.name} px: no text overflows`, async ({ page }) => {
    await page.setViewportSize(viewport);
    await fakeSession(page, sessionWithRole("member"));
    await fakeNotice(page, null);
    await page.goto("/privacy?pseudo");
    await expect(page.getByRole("heading", { level: 1, name: /Dátéñsçhútz/ })).toBeVisible();
    await expect(page.locator("[role=status][aria-label]")).toHaveCount(0);

    expect(await textOverflows(page)).toEqual([]);
  });
}

test("the footer and the member menu lead to the privacy notice", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await fakeSession(page, sessionWithRole("member"));
  await fakeNotice(page, null);
  await page.goto("/privacy");
  await page.getByRole("link", { name: "Datenschutz" }).click();
  await expect(page).toHaveURL(/\/privacy$/);

  await page.setViewportSize({ width: 375, height: 812 });
  await fakeEvents(page, 200, { items: [] });
  await page.goto("/events");
  await page.getByRole("button", { name: "Anna Muster" }).click();
  await page.getByRole("menuitem", { name: "Datenschutz" }).click();
  await expect(page).toHaveURL(/\/privacy$/);
});
