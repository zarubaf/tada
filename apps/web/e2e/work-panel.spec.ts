import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";
import { fontsLoaded, setTheme, themes, viewports } from "./fixtures";
import { actions, event, fakeWork } from "./workFixtures";

// The side panel of the „Arbeit“ page (ADR 0082). The unit tests hold the rules; these checks hold
// what only a browser shows: the docked column and the full-screen sheet, the inert page behind the
// sheet, the 44 px targets on touch, the focus rules and the screenshots.
const work = `/events/${event?.id}/work`;

const axe = (page: Page) =>
  new AxeBuilder({ page }).withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"]).analyze();

// ACT-001 with everything the panel can show: a description, a start date, a milestone, the next
// statuses and a source.
const panelActions = actions.map((action: Record<string, unknown>) =>
  action.local_id === "ACT-001"
    ? {
        ...action,
        description:
          "Zwei Angebote einholen und mit dem Budget vergleichen.\nDie Entscheidung fällt an der nächsten Sitzung.",
        start_date: "2020-05-01",
        milestone_id: "0199b8e0-0000-7000-8000-000000000121",
        next_statuses: ["in-progress", "blocked", "done", "canceled"],
        evidence: [
          {
            proposal_id: "0199b8e0-0000-7000-8000-000000000501",
            record_version: 1,
            source_version_id: "0199b8e0-0000-7000-8000-000000000601",
            captured_at: "2030-03-01T10:00:00Z",
            start_offset: 0,
            end_offset: 40,
            quote: "Marco, kannst du bis Mitte Mai den Generator anfragen?",
            redacted: false,
          },
        ],
      }
    : action,
);

async function open(page: Page, search: string) {
  await fakeWork(page, { actions: panelActions });
  // The baselines must not depend on the real date: it decides what is overdue.
  await page.clock.setFixedTime(new Date("2030-05-08T10:00:00Z"));
  await page.goto(`${work}${search}`);
  await expect(page.getByRole("heading", { level: 1, name: "Arbeit" })).toBeVisible();
}

for (const viewport of viewports) {
  const narrow = viewport.width < 640;
  for (const theme of themes) {
    const name = `${theme}, ${viewport.name} px`;

    test(`open on an action, ${name}: no axe violation, screenshot`, async ({ page }) => {
      await page.setViewportSize(viewport);
      await open(page, "?view=table#ACT-001");
      await setTheme(page, theme);
      const panel = page.getByRole("complementary", { name: /^ACT-001/ });
      await expect(panel).toBeVisible();
      await expect(panel.getByRole("heading", { level: 3, name: "Quellen" })).toBeVisible();
      // The panel slides in; measure it when it has stopped.
      await expect
        .poll(async () => (await panel.boundingBox())?.x)
        .toBe(narrow ? 0 : viewport.width - 480);
      const box = await panel.boundingBox();
      if (narrow) {
        // The sheet covers the screen.
        expect(box).toMatchObject({ x: 0, width: viewport.width });
        expect(box?.height).toBeGreaterThanOrEqual(viewport.height - 1);
      } else {
        // The panel is a column beside the list, which stays usable.
        expect(box?.width).toBeLessThan(viewport.width / 2);
        expect(box?.x).toBeGreaterThan(viewport.width / 2);
        const list = await page
          .getByRole("table")
          .or(page.getByRole("region"))
          .first()
          .boundingBox();
        expect((list?.x ?? 0) + (list?.width ?? 0)).toBeLessThanOrEqual(box?.x ?? 0);
      }
      expect((await axe(page)).violations).toEqual([]);
      await fontsLoaded(page);
      await expect(page).toHaveScreenshot(`panel-open-${theme}-${viewport.name}.png`);
    });

    test(`a field in edit state, ${name}: no axe violation, screenshot`, async ({ page }) => {
      await page.setViewportSize(viewport);
      await open(page, "#ACT-001");
      await setTheme(page, theme);
      const panel = page.getByRole("complementary", { name: /^ACT-001/ });
      await panel.getByRole("button", { name: /^Titel bearbeiten/ }).click();
      await expect(panel.getByRole("textbox", { name: "Titel" })).toBeFocused();
      expect((await axe(page)).violations).toEqual([]);
      await fontsLoaded(page);
      await expect(page).toHaveScreenshot(`panel-edit-${theme}-${viewport.name}.png`);
    });

    test(`create state, ${name}: no axe violation, screenshot`, async ({ page }) => {
      await page.setViewportSize(viewport);
      await open(page, "?new=action");
      await setTheme(page, theme);
      const panel = page.getByRole("complementary", { name: "Neue Aufgabe" });
      await expect(panel.getByRole("textbox", { name: /^Titel/ })).toBeVisible();
      expect((await axe(page)).violations).toEqual([]);
      await fontsLoaded(page);
      await expect(page).toHaveScreenshot(`panel-create-${theme}-${viewport.name}.png`);
    });
  }
}

test("Escape ends an edit first and closes the panel second, and focus returns to the card", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await open(page, "");
  const card = page.getByRole("link", { name: /ACT-001/ });
  await card.click();
  const panel = page.getByRole("complementary", { name: /^ACT-001/ });
  await panel.getByRole("button", { name: /^Titel bearbeiten/ }).click();
  await page.keyboard.type(" Entwurf");
  await page.keyboard.press("Escape");
  await expect(panel).toBeVisible();
  await expect(panel.getByRole("button", { name: "Titel bearbeiten" })).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("complementary")).toHaveCount(0);
  await expect(card).toBeFocused();
  expect(new URL(page.url()).hash).toBe("");
});

test("an edit in the panel changes the card in the list", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await open(page, "#ACT-001");
  await page.route("**/api/v1/events/*/actions/*", async (route) => {
    const request = route.request();
    if (request.method() !== "PATCH") {
      return route.fallback();
    }
    const { expected_version: _, ...fields } = request.postDataJSON();
    return route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ ...panelActions[0], ...fields, version: 2 }),
    });
  });
  const panel = page.getByRole("complementary", { name: /^ACT-001/ });
  await panel.getByRole("button", { name: /^Titel bearbeiten/ }).click();
  await page.keyboard.press("ControlOrMeta+A");
  await page.keyboard.type("Generator anfragen");
  await page.keyboard.press("Enter");
  await expect(page.getByRole("link", { name: "ACT-001 Generator anfragen" })).toBeVisible();
});

test.describe("on a touch screen", () => {
  test.use({ viewport: { width: 375, height: 812 }, hasTouch: true, isMobile: true });

  test("every control of the sheet is 44 px high, and the page behind is inert", async ({
    page,
  }) => {
    await open(page, "#ACT-001");
    const panel = page.getByRole("complementary", { name: /^ACT-001/ });
    await expect(panel).toBeVisible();
    const controls = panel.locator("button, input, textarea, a[href]");
    const heights = await controls.evaluateAll((elements) =>
      elements.map((element) => ({
        name: element.getAttribute("aria-label") ?? element.textContent?.trim().slice(0, 30),
        height: element.getBoundingClientRect().height,
      })),
    );
    expect(heights.length).toBeGreaterThan(6);
    for (const { name, height } of heights) {
      expect([name, height >= 44]).toEqual([name, true]);
    }
    // The live regions of the page stay readable, so the inert marks sit on the parts around them.
    const inert = await page.evaluate(
      () => document.querySelector("main h1")?.closest("[inert]") !== null,
    );
    expect(inert).toBe(true);
  });
});
