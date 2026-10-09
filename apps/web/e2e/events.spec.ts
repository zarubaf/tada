import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";
import {
  events,
  fakeEvents,
  fakeSession,
  fontsLoaded,
  sessionInfo,
  setTheme,
  themes,
  unavailable,
  viewports,
} from "./fixtures";

const states = [
  { name: "list", status: 200, body: { items: events } },
  { name: "empty", status: 200, body: { items: [] } },
  { name: "error", status: 503, body: unavailable },
] as const;

for (const viewport of viewports) {
  for (const theme of themes) {
    for (const state of states) {
      test(`events, ${state.name}, ${theme}, ${viewport.name} px: no axe violation`, async ({
        page,
      }) => {
        await page.setViewportSize(viewport);
        await fakeSession(page);
        await fakeEvents(page, state.status, state.body);
        await page.goto("/events");
        await setTheme(page, theme);
        await expect(page.getByRole("heading", { name: "Anlässe" })).toBeVisible();
        // The skeleton is a labelled status; the live regions have no label.
        await expect(page.locator("[role=status][aria-label]")).toHaveCount(0);

        const results = await new AxeBuilder({ page })
          .withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"])
          .analyze();
        expect(results.violations).toEqual([]);
      });
    }

    test(`events, two organizations, ${theme}, ${viewport.name} px: no axe violation`, async ({
      page,
    }) => {
      const other = { ...sessionInfo.organization, organization_id: "x", name: "Segelflugclub" };
      await page.setViewportSize(viewport);
      await fakeSession(page, { ...sessionInfo, memberships: [sessionInfo.organization, other] });
      await fakeEvents(page, 200, { items: events });
      await page.goto("/events");
      await setTheme(page, theme);
      await expect(page.getByRole("button", { name: /Organisation/ })).toBeVisible();
      await expect(page.getByRole("table")).toBeVisible();

      const results = await new AxeBuilder({ page })
        .withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"])
        .analyze();
      expect(results.violations).toEqual([]);
    });

    test(`events, list, ${theme}, ${viewport.name} px: screenshot`, async ({ page }) => {
      await page.setViewportSize(viewport);
      await fakeSession(page);
      await fakeEvents(page, 200, { items: events });
      await page.goto("/events");
      await setTheme(page, theme);
      await expect(page.getByRole("table")).toBeVisible();
      await fontsLoaded(page);
      await expect(page).toHaveScreenshot(`events-${theme}-${viewport.name}.png`, {
        fullPage: true,
      });
    });
  }
}

test("the e2e build has the gallery route", async ({ page }) => {
  await page.goto("/_gallery");
  await expect(page.getByRole("heading", { name: "Komponenten" })).toBeVisible();
});

test("in forced colors the current navigation item keeps a mark", async ({ page }) => {
  await page.emulateMedia({ forcedColors: "active" });
  await fakeSession(page);
  await fakeEvents(page, 200, { items: events });
  await page.goto("/events");

  const current = page.getByRole("link", { name: "Anlässe" });
  await expect(current).toHaveAttribute("aria-current", "page");
  await expect(current).toHaveCSS("text-decoration-line", "underline");
  await expect(page.getByRole("link", { name: "Einstellungen" })).toHaveCSS(
    "text-decoration-line",
    "none",
  );
});

for (const theme of themes) {
  test(`with more contrast, ${theme}: muted text and subtle borders take the strong values`, async ({
    page,
  }) => {
    await page.emulateMedia({ contrast: "more" });
    await fakeSession(page);
    await fakeEvents(page, 200, { items: events });
    await page.goto("/events");
    await setTheme(page, theme);

    const values = await page.evaluate(() => {
      const resolve = (name: string) => {
        const probe = document.createElement("span");
        probe.style.color = `var(${name})`;
        document.body.append(probe);
        const color = getComputedStyle(probe).color;
        probe.remove();
        return color;
      };
      return {
        muted: resolve("--color-text-muted"),
        text: resolve("--color-text"),
        subtle: resolve("--color-border-subtle"),
        control: resolve("--color-border-control"),
      };
    });
    expect(values.muted).toBe(values.text);
    expect(values.subtle).toBe(values.control);
  });
}
