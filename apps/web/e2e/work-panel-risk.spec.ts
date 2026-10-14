import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";
import { fontsLoaded, setTheme, themes, viewports } from "./fixtures";
import { event, fakeWork, risks } from "./workFixtures";

// The risk in the side panel of the „Arbeit“ page (ADR 0082, 0081). The unit tests hold the rules;
// these checks hold what only a browser shows: the rating as words with an icon in the panel and
// on the card, the level select, the create state, and the screenshots.
const work = `/events/${event?.id}/work`;

const axe = (page: Page) =>
  new AxeBuilder({ page }).withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"]).analyze();

// RSK-001 with everything the panel can show: the rating, the texts, the next statuses and a source.
const panelRisks = risks.map((risk: Record<string, unknown>) =>
  risk.local_id === "RSK-001"
    ? {
        ...risk,
        impact: "high",
        likelihood: "medium",
        status: "open",
        description:
          "Die Turnshow findet im Freien statt.\nBei Starkregen fällt sie aus, und die Gäste fehlen am Nachmittag.",
        mitigation: "Das Festzelt als Ausweichraum reservieren.",
        next_statuses: ["mitigated", "occurred", "closed"],
        evidence: [
          {
            proposal_id: "0199b8e0-0000-7000-8000-000000000501",
            record_version: 1,
            source_version_id: "0199b8e0-0000-7000-8000-000000000601",
            captured_at: "2030-03-01T10:00:00Z",
            start_offset: 0,
            end_offset: 40,
            quote: "Wenn es regnet, haben wir keinen Ausweichraum für die Turnshow.",
            redacted: false,
          },
        ],
      }
    : risk,
);

async function open(page: Page, search: string) {
  await fakeWork(page, { risks: panelRisks });
  await page.clock.setFixedTime(new Date("2030-05-08T10:00:00Z"));
  await page.goto(`${work}${search}`);
  await expect(page.getByRole("heading", { level: 1, name: "Arbeit" })).toBeVisible();
}

for (const viewport of viewports) {
  const narrow = viewport.width < 640;
  for (const theme of themes) {
    const name = `${theme}, ${viewport.name} px`;

    test(`open on a risk, ${name}: no axe violation, screenshot`, async ({ page }) => {
      await page.setViewportSize(viewport);
      await open(page, "?view=table#RSK-001");
      await setTheme(page, theme);
      const panel = page.getByRole("complementary", { name: /^RSK-001/ });
      await expect(panel).toBeVisible();
      await expect(panel.getByRole("button", { name: "Wirkung: hoch" })).toBeVisible();
      await expect(panel.getByRole("button", { name: "Wahrscheinlichkeit: mittel" })).toBeVisible();
      await expect(panel.getByRole("heading", { level: 3, name: "Gegenmassnahme" })).toBeVisible();
      await expect
        .poll(async () => (await panel.boundingBox())?.x)
        .toBe(narrow ? 0 : viewport.width - 480);
      // Every label ends before its value starts, also the long „Wahrscheinlichkeit“.
      const overlaps = await panel
        .locator("dl > dt")
        .evaluateAll((labels) =>
          labels.flatMap((label) =>
            label.scrollWidth > label.clientWidth + 1 ? [label.textContent] : [],
          ),
        );
      expect(overlaps).toEqual([]);
      expect((await axe(page)).violations).toEqual([]);
      await fontsLoaded(page);
      await expect(page).toHaveScreenshot(`panel-risk-open-${theme}-${viewport.name}.png`);
    });

    test(`the level select is open, ${name}: no axe violation, screenshot`, async ({ page }) => {
      await page.setViewportSize(viewport);
      await open(page, "#RSK-001");
      await setTheme(page, theme);
      const panel = page.getByRole("complementary", { name: /^RSK-001/ });
      await panel.getByRole("button", { name: "Wirkung: hoch" }).click();
      const list = page.getByRole("listbox", { name: "Wirkung" });
      await expect(list.getByRole("option")).toHaveText(["niedrig", "mittel", "hoch"]);
      expect((await axe(page)).violations).toEqual([]);
      await fontsLoaded(page);
      await expect(page).toHaveScreenshot(`panel-risk-level-${theme}-${viewport.name}.png`);
    });

    test(`create state, ${name}: no axe violation, screenshot`, async ({ page }) => {
      await page.setViewportSize(viewport);
      await open(page, "?new=risk");
      await setTheme(page, theme);
      const panel = page.getByRole("complementary", { name: "Neues Risiko" });
      await expect(panel.getByRole("textbox", { name: /^Titel/ })).toBeVisible();
      await expect(
        panel.getByRole("button", { name: "Wirkung (Pflichtfeld): Stufe wählen" }),
      ).toBeVisible();
      expect((await axe(page)).violations).toEqual([]);
      await fontsLoaded(page);
      await expect(page).toHaveScreenshot(`panel-risk-create-${theme}-${viewport.name}.png`);
    });
  }
}

test("the values of a risk start at the same x as those of an action", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const valueX = async (name: RegExp) => {
    const panel = page.getByRole("complementary", { name });
    await expect(panel).toBeVisible();
    return panel
      .locator("dl > dd")
      .first()
      .evaluate((value) => value.getBoundingClientRect().x);
  };
  await open(page, "?view=table#ACT-001");
  const action = await valueX(/^ACT-001/);
  await page.goto(`${work}?view=table#RSK-001`);
  const risk = await valueX(/^RSK-001/);
  expect(risk).toBe(action);
});

test("the card of a risk links to the panel and shows the rating in words", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await open(page, "");
  const card = page.getByRole("link", { name: /RSK-001/ });
  await expect(card).toContainText("Wirkung hoch");
  await expect(card).toContainText("Wahrscheinlich mittel");
  await card.click();
  await expect(page.getByRole("complementary", { name: /^RSK-001/ })).toBeVisible();
  expect(new URL(page.url()).hash).toBe("#RSK-001");
});

test("a level changes with the keyboard and the card follows", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await open(page, "#RSK-001");
  await page.route("**/api/v1/events/*/risks/*", async (route) => {
    const request = route.request();
    if (request.method() !== "PATCH") {
      return route.fallback();
    }
    const { expected_version: _, ...fields } = request.postDataJSON();
    return route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ ...panelRisks[0], ...fields, version: 2 }),
    });
  });
  const panel = page.getByRole("complementary", { name: /^RSK-001/ });
  await panel.getByRole("button", { name: "Wirkung: hoch" }).focus();
  await page.keyboard.press("Enter");
  await expect(page.getByRole("listbox", { name: "Wirkung" })).toBeVisible();
  await page.keyboard.press("ArrowUp");
  await page.keyboard.press("Enter");
  await expect(panel.getByRole("button", { name: "Wirkung: mittel" })).toBeFocused();
  await expect(page.getByRole("link", { name: /RSK-001/ })).toContainText("Wirkung mittel");
});

test.describe("on a touch screen", () => {
  test.use({ viewport: { width: 375, height: 812 }, hasTouch: true, isMobile: true });

  test("every control of the risk sheet is 44 px high", async ({ page }) => {
    await open(page, "#RSK-001");
    const panel = page.getByRole("complementary", { name: /^RSK-001/ });
    await expect(panel).toBeVisible();
    const heights = await panel
      .locator("button, input, textarea, a[href]")
      .evaluateAll((elements) =>
        elements.map((element) => ({
          name: element.getAttribute("aria-label") ?? element.textContent?.trim().slice(0, 30),
          height: element.getBoundingClientRect().height,
        })),
      );
    expect(heights.length).toBeGreaterThan(6);
    for (const { name, height } of heights) {
      expect([name, height >= 44]).toEqual([name, true]);
    }
  });
});
