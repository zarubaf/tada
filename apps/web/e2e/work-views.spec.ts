import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";
import { fakeProfile, fontsLoaded, setTheme, textOverflows, themes, viewports } from "./fixtures";
import { actions, event, fakeWork } from "./workFixtures";

const work = `/events/${event?.id}/work`;

const axe = (page: Page) =>
  new AxeBuilder({ page }).withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"]).analyze();

/** An event window around the dates of the records, so that the plan has a band. */
const eventWindow = {
  id: "f1",
  field_id: "fd1",
  field_key: "date_window",
  state: "assumption",
  value: { type: "date-window", start: "2099-05-14", end: "2099-06-02", granularity: "day" },
  redacted: false,
  version: 1,
  evidence: [],
  accepted_by: { kind: "member", id: "x", channel: "web" },
  accepted_at: "2030-03-01T13:12:00Z",
};

// The sample has two actions in 2020, which stretch the axis over 80 years: the timeline shows the
// same actions near the others, so that the screenshot shows the plan and not an empty axis.
const near = actions.map((action: Record<string, unknown>) =>
  String(action.due_date).startsWith("2020") ? { ...action, due_date: "2099-05-10" } : action,
);

async function open(page: Page, search: string) {
  await fakeWork(page, search.includes("view=timeline") ? { actions: near } : {});
  await fakeProfile(page, { facts: [eventWindow], proposals: [], open_questions: [] }, []);
  // The baselines must not depend on the real date: today is on the axis and decides what is overdue.
  await page.clock.setFixedTime(new Date("2030-05-08T10:00:00Z"));
  await page.goto(`${work}${search}`);
  await expect(page.getByRole("heading", { level: 1, name: "Arbeit" })).toBeVisible();
}

async function tableShown(page: Page, narrow: boolean) {
  if (narrow) {
    await expect(page.getByRole("link", { name: /ACT-001/ })).toBeVisible();
    await expect(page.getByRole("table")).toHaveCount(0);
  } else {
    // Where the table scrolls, its region carries the name, and where it fits, the table does.
    await expect(
      page
        .getByRole("table")
        .or(page.getByRole("region", { name: "Arbeit als Tabelle" }))
        .first(),
    ).toBeVisible();
  }
  await expect(page.getByRole("status", { name: "Arbeit wird geladen" })).toHaveCount(0);
}

async function timelineShown(page: Page, narrow: boolean) {
  if (narrow) {
    await expect(page.getByRole("region", { name: "Nach Wochen" })).toBeVisible();
  } else {
    await expect(page.getByRole("region", { name: "Zeitachse" })).toBeVisible();
  }
}

for (const viewport of viewports) {
  const narrow = viewport.width < 640;
  for (const theme of themes) {
    test(`table, ${theme}, ${viewport.name} px: no axe violation, screenshot`, async ({ page }) => {
      await page.setViewportSize(viewport);
      await open(page, "?view=table");
      await setTheme(page, theme);
      await tableShown(page, narrow);
      expect((await axe(page)).violations).toEqual([]);
      await fontsLoaded(page);
      await expect(page).toHaveScreenshot(`work-table-${theme}-${viewport.name}.png`, {
        fullPage: true,
      });
    });

    test(`timeline view, ${theme}, ${viewport.name} px: no axe violation, screenshot`, async ({
      page,
    }) => {
      await page.setViewportSize(viewport);
      await open(page, "?view=timeline");
      await setTheme(page, theme);
      await timelineShown(page, narrow);
      expect((await axe(page)).violations).toEqual([]);
      await fontsLoaded(page);
      await expect(page).toHaveScreenshot(`work-timeline-${theme}-${viewport.name}.png`, {
        fullPage: true,
      });
    });
  }
}

test("the tabs switch the view, keep it in the URL and survive a reload", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await open(page, "?type=action");
  await expect(page.getByRole("tab", { name: "Board", selected: true })).toBeVisible();
  await page.getByRole("tab", { name: "Tabelle" }).click();
  await tableShown(page, false);
  expect(new URL(page.url()).searchParams.get("view")).toBe("table");
  await page.getByRole("columnheader", { name: "Titel" }).getByRole("button").click();
  await expect(page.getByRole("columnheader", { name: "Titel" })).toHaveAttribute(
    "aria-sort",
    "ascending",
  );
  await page.reload();
  await tableShown(page, false);
  await expect(page.getByRole("columnheader", { name: "Titel" })).toHaveAttribute(
    "aria-sort",
    "ascending",
  );
  await page.getByRole("tab", { name: "Zeitplan" }).click();
  await timelineShown(page, false);
  await page.goBack();
  await tableShown(page, false);
  await page.goBack();
  await expect(page.getByRole("tab", { name: "Board", selected: true })).toBeVisible();
  expect(new URL(page.url()).searchParams.has("view")).toBe(false);
  expect(new URL(page.url()).searchParams.get("type")).toBe("action");
});

test("the sidebar item Zeitplan is current in the timeline view and Arbeit is not", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await open(page, "?view=timeline");
  const nav = page.getByRole("navigation", { name: "Hauptnavigation" });
  await expect(nav.getByRole("link", { name: "Zeitplan" })).toHaveAttribute("aria-current", "page");
  await expect(nav.getByRole("link", { name: "Arbeit", exact: true })).not.toHaveAttribute(
    "aria-current",
  );
  await nav.getByRole("link", { name: "Arbeit", exact: true }).click();
  await expect(page.getByRole("tab", { name: "Board", selected: true })).toBeVisible();
  await expect(nav.getByRole("link", { name: "Arbeit", exact: true })).toHaveAttribute(
    "aria-current",
    "page",
  );
  await nav.getByRole("link", { name: "Zeitplan" }).click();
  await expect(page.getByRole("tab", { name: "Zeitplan", selected: true })).toBeVisible();
});

test("a title in the table is one link, also for a risk", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await open(page, "?view=table");
  await tableShown(page, false);
  const row = page.getByRole("row").filter({ hasText: "ACT-001" });
  await expect(row.getByRole("link")).toHaveAttribute(
    "href",
    `/events/${event?.id}/work?view=table#ACT-001`,
  );
  await expect(
    page.getByRole("row").filter({ hasText: "RSK-001" }).getByRole("link"),
  ).toHaveAttribute("href", `/events/${event?.id}/work?view=table#RSK-001`);
});

test("the table rows are 28 px high with a fine pointer", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await open(page, "?view=table");
  await tableShown(page, false);
  const heights = await page
    .locator("tbody tr")
    .evaluateAll((rows) => rows.map((row) => Math.round(row.getBoundingClientRect().height)));
  expect(heights.length).toBeGreaterThan(0);
  expect(heights.every((height) => height === 29 || height === 28)).toBe(true);
});

// ADR 0022: the targets are 44 px with a coarse pointer, also in the dense table.
test.describe("on a touch screen between the phone and the docked table", () => {
  test.use({ hasTouch: true, isMobile: true });

  test("every control of the table is 44 px high and the page does not scroll sideways", async ({
    page,
  }) => {
    await page.setViewportSize({ width: 700, height: 900 });
    await open(page, "?view=table");
    await tableShown(page, false);
    const small: string[] = [];
    for (const target of await page.getByRole("table").locator("button, a").all()) {
      // A secondary column leaves a table in a container narrower than 56 rem; it has no target.
      if (!(await target.isVisible())) {
        continue;
      }
      const height = (await target.boundingBox())?.height ?? 0;
      if (height < 44) {
        small.push(`${await target.textContent()} ${height}`);
      }
    }
    expect(small).toEqual([]);
    expect(
      await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth),
    ).toBe(true);
  });
});

for (const viewport of viewports) {
  test(`pseudo-locale, table and timeline, ${viewport.name} px: no text overflows`, async ({
    page,
  }) => {
    await page.setViewportSize(viewport);
    await open(page, "");
    for (const view of ["table", "timeline"]) {
      await page.goto(`${work}?view=${view}&pseudo`);
      // The pseudo-locale changes the words, so a link to a record is the sign that the view shows.
      await expect(
        page.locator("a[href$='#ACT-001'], a[href$='#MIL-001']").filter({ visible: true }).first(),
      ).toBeVisible();
      expect(await textOverflows(page)).toEqual([]);
    }
  });
}

test("at 320 px the table view and the timeline view do not scroll sideways", async ({ page }) => {
  await page.setViewportSize({ width: 320, height: 800 });
  await open(page, "?view=table");
  await tableShown(page, true);
  const fits = () => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth);
  expect(await fits()).toBe(true);
  await page.goto(`${work}?view=timeline`);
  await timelineShown(page, true);
  expect(await fits()).toBe(true);
});

// With the column of the checkboxes, the columns need more room than the container has at some
// widths. The container clips, so the table must scroll inside it there, never lose a column.
for (const width of [800, 900, 1024, 1100]) {
  test(`table, ${width} px with the sidebar open: no column is cut off`, async ({ page }) => {
    await page.setViewportSize({ width, height: 900 });
    await open(page, "?view=table");
    await tableShown(page, false);
    const cut = await page.getByRole("table").evaluate((table) => {
      const container = table.closest("[class*='container']") as HTMLElement;
      const scroller = table.closest("[class*='scroller']") as HTMLElement;
      const scrolls = getComputedStyle(scroller).overflowX === "auto";
      const overshoot =
        table.getBoundingClientRect().right - container.getBoundingClientRect().right;
      return { scrolls, overshoot: Math.round(overshoot) };
    });
    // Either the table fits, or its scroller is on and shows the rest.
    expect(cut.overshoot <= 0 || cut.scrolls).toBe(true);
  });
}

test("a bar of the timeline opens its record in the panel", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await open(page, "?view=timeline");
  await page
    .getByRole("region", { name: "Zeitachse" })
    .getByRole("link", { name: /ACT-001/ })
    .click();
  await expect(page).toHaveURL(/\/work(\?.*)?#ACT-001$/);
  await expect(page.getByRole("complementary", { name: /ACT-001/ })).toBeVisible();
});

test("the skip link moves focus to the main region and opens no record", async ({ page }) => {
  await open(page, "?view=table");
  await page.keyboard.press("Tab");
  const skip = page.getByRole("link", { name: "Zum Inhalt springen" });
  await expect(skip).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page.locator("main#main")).toBeFocused();
  await expect(page.getByRole("complementary")).toHaveCount(0);
  expect(new URL(page.url()).hash).toBe("");
});
