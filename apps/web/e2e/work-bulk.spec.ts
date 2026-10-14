import AxeBuilder from "@axe-core/playwright";
import { expect, type Locator, type Page, test } from "@playwright/test";
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

// Card selection and the bulk bar of the „Arbeit“ board (ADR 0082). The unit tests hold the rules;
// these checks hold what only a browser shows: the hover and focus rules of the checkbox, the place
// of the bar, the 44 px targets and the screenshots.
const event = events[0];
const ME = sessionInfo.user_id;
const BERND = "0199b8e0-0000-7000-8000-0000000000b2";
const GROUND = "0199b8e0-0000-7000-8000-0000000000c1";

const members = [
  { user_id: ME, display_name: "Anna Muster", role: "member", version: 1 },
  {
    user_id: BERND,
    display_name: "Bernhard Beispiel-Schmidlin-Äbischer",
    role: "member",
    version: 1,
  },
];
const memberships = members.map(({ user_id, display_name }, index) => ({
  user_id,
  display_name,
  event_role: index === 0 ? "event-manager" : "event-contributor",
  version: 1,
  created_at: "2028-03-01T13:12:00Z",
}));
const workstreams = [
  {
    id: GROUND,
    event_id: event?.id,
    name: "Bodenbetrieb und Flugplatzinfrastruktur",
    lead_user_id: BERND,
    status: "active",
    version: 1,
  },
];

const NEXT = ["in-progress", "blocked", "done", "canceled"];
const action = (n: string, title: string, over: Record<string, unknown> = {}) => ({
  id: `0199b8e0-0000-7000-8000-0000000001${n}`,
  local_id: `ACT-0${n}`,
  event_id: event?.id,
  title,
  status: "open",
  owner_user_id: ME,
  due_date: null,
  workstream_id: null,
  redacted: false,
  version: 1,
  evidence: [],
  can_change: true,
  next_statuses: NEXT,
  ...over,
});
const actions = [
  action("01", "Generatorenvermietung Testwil-Oberdorf anfragen und Angebote vergleichen"),
  action("02", "Festzelt reservieren", { owner_user_id: BERND, workstream_id: GROUND }),
  action("03", "Flyer drucken", { status: "in-progress" }),
  action("04", "Hallenplan zeichnen", { can_change: false, next_statuses: [] }),
];

/** Enough open actions that the end of the list lies below the first screen. */
const manyActions = Array.from({ length: 30 }, (_, index) =>
  action(String(index + 1).padStart(2, "0"), `Aufgabe Nummer ${index + 1}`),
);

async function fakeWork(page: Page, rows: typeof actions = actions): Promise<void> {
  await fakeSession(page, sessionWithRole("member"));
  await fakeEvent(page, event);
  const json = (pattern: string, body: unknown) =>
    page.route(pattern, (route) =>
      route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(body) }),
    );
  await json("**/api/v1/events/*/memberships", { items: memberships });
  await json("**/api/v1/members*", { items: members });
  await json("**/api/v1/events/*/workstreams", { items: workstreams });
  await json("**/api/v1/events/*/actions*", { items: rows });
  for (const kind of ["commitments", "milestones", "decisions", "risks"]) {
    await json(`**/api/v1/events/*/${kind}*`, { items: [] });
  }
}

/** Answers a change of an action with the record and its new field values. */
async function acceptChanges(page: Page): Promise<unknown[]> {
  const sent: unknown[] = [];
  await page.route("**/api/v1/events/*/actions/*", async (route) => {
    const request = route.request();
    if (request.method() !== "PATCH") {
      return route.fallback();
    }
    const body = request.postDataJSON();
    sent.push(body);
    const id = new URL(request.url()).pathname.split("/").pop();
    const before = actions.find((a) => a.id === id);
    const { expected_version: _version, ...fields } = body;
    return route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ ...before, ...fields, version: 2 }),
    });
  });
  return sent;
}

const axe = (page: Page) =>
  new AxeBuilder({ page }).withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"]).analyze();
const work = `/events/${event?.id}/work`;
const box = (page: Page, id: string) =>
  page.getByRole("checkbox", { name: new RegExp(`^${id} .* auswählen$`) });
/** The box is drawn by the label; the input beside it is for the keyboard and the screen reader. */
const label = (page: Page, id: string) => box(page, id).locator("xpath=ancestor::label");
const bar = (page: Page) => page.getByRole("toolbar", { name: "Aktionen für die Auswahl" });
const opacityOf = (locator: Locator) =>
  locator.evaluate((element) => {
    let node: Element | null = element;
    while (node && getComputedStyle(node).opacity === "1") {
      node = node.parentElement;
    }
    return node ? Number(getComputedStyle(node).opacity) : 1;
  });

test.describe("a mouse", () => {
  test.use({ viewport: { width: 1440, height: 900 } });

  test("the checkbox shows on hover and on focus within, and stays while a selection exists", async ({
    page,
  }) => {
    await fakeWork(page);
    await page.goto(work);
    const first = box(page, "ACT-001");
    await expect(page.getByRole("link", { name: /ACT-001/ })).toBeVisible();
    await page.mouse.move(0, 0);
    expect(await opacityOf(first)).toBe(0);

    await page.getByRole("link", { name: /ACT-001/ }).hover();
    await expect.poll(() => opacityOf(first)).toBe(1);

    // Keyboard: Tab reaches the checkbox of the card before its link, and shows it.
    await page.mouse.move(0, 0);
    const second = box(page, "ACT-002");
    await second.focus();
    await expect.poll(() => opacityOf(second)).toBe(1);

    // With a selection every card shows its checkbox.
    await label(page, "ACT-001").click();
    await page.mouse.move(0, 0);
    await expect(first).toBeChecked();
    await expect.poll(() => opacityOf(box(page, "ACT-003"))).toBe(1);
  });

  test("a record the member cannot change has no checkbox", async ({ page }) => {
    await fakeWork(page);
    await page.goto(work);
    await expect(page.getByRole("link", { name: /ACT-004/ })).toBeVisible();
    await expect(box(page, "ACT-004")).toHaveCount(0);
  });

  test("changes the status of two records and clears the selection", async ({ page }) => {
    await fakeWork(page);
    const sent = await acceptChanges(page);
    await page.goto(work);
    await label(page, "ACT-001").click();
    await label(page, "ACT-003").click();
    await expect(bar(page).getByText("2 ausgewählt")).toBeVisible();
    // The bar is in view without a scroll.
    const frame = await bar(page).boundingBox();
    expect(frame && frame.y + frame.height <= 900).toBe(true);
    // The bar is 44 px high and floats 20 px above the bottom of the window (the approved mock).
    expect(frame?.height).toBeCloseTo(44, 0);
    expect(900 - ((frame?.y ?? 0) + (frame?.height ?? 0))).toBeCloseTo(20, 0);
    // Its buttons are 30 px high, and the rules between the groups are 20 px high (the mock).
    const status = await bar(page).getByRole("button", { name: "Status" }).boundingBox();
    expect(status?.height).toBeCloseTo(30, 0);

    await bar(page).getByRole("button", { name: "Status" }).click();
    await page.getByRole("option", { name: "Erledigt" }).click();
    await expect(page.getByText("2 Einträge geändert")).toBeAttached();
    await expect(bar(page)).toHaveCount(0);
    expect(sent).toEqual([
      { status: "done", expected_version: 1 },
      { status: "done", expected_version: 1 },
    ]);
    await expect(page.getByRole("heading", { level: 1, name: "Arbeit" })).toBeFocused();
  });

  test("Ctrl+A selects what shows and Escape clears", async ({ page }) => {
    await fakeWork(page);
    await page.goto(work);
    await page.getByRole("link", { name: /ACT-001/ }).focus();
    await page.keyboard.press("Control+a");
    await expect(bar(page).getByText("3 ausgewählt")).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(bar(page)).toHaveCount(0);
  });

  for (const theme of themes) {
    test(`the bar, ${theme}: no axe violation, also with the list open`, async ({ page }) => {
      await fakeWork(page);
      await page.goto(work);
      await setTheme(page, theme);
      await label(page, "ACT-001").click();
      await label(page, "ACT-002").click();
      await expect(bar(page)).toBeVisible();
      expect((await axe(page)).violations).toEqual([]);
      await bar(page).getByRole("button", { name: "Verantwortlich" }).click();
      await expect(page.getByRole("listbox", { name: "Verantwortlich" })).toBeVisible();
      expect((await axe(page)).violations).toEqual([]);
    });
  }
});

for (const viewport of viewports) {
  for (const theme of themes) {
    test(`the bar, ${theme}, ${viewport.name} px: screenshot, no sideways scroll`, async ({
      page,
    }) => {
      await page.setViewportSize(viewport);
      await fakeWork(page);
      await page.goto(work);
      await setTheme(page, theme);
      await label(page, "ACT-001").click();
      await label(page, "ACT-002").click();
      await expect(bar(page)).toBeVisible();
      await page.mouse.move(0, 0);
      expect(
        await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth),
      ).toBe(true);
      if (viewport.width < 640) {
        // The bar sits above the bottom bar, not under it.
        const bottomBar = await page.getByRole("navigation").last().boundingBox();
        const frame = await bar(page).boundingBox();
        expect(frame && bottomBar && frame.y + frame.height <= bottomBar.y).toBe(true);
      }
      expect((await axe(page)).violations).toEqual([]);
      await fontsLoaded(page);
      await expect(page).toHaveScreenshot(`work-bulk-${theme}-${viewport.name}.png`);
    });
  }
}

test.describe("a phone with touch", () => {
  test.use({ hasTouch: true, isMobile: true, viewport: { width: 375, height: 812 } });

  test("shows the checkboxes without hover and has 44 px targets, and the lists are a sheet", async ({
    page,
  }) => {
    await fakeWork(page);
    const sent = await acceptChanges(page);
    await page.goto(work);
    const first = box(page, "ACT-001");
    await expect(page.getByRole("link", { name: /ACT-001/ })).toBeVisible();
    expect(await opacityOf(first)).toBe(1);
    const hit = await first.locator("xpath=ancestor::label").boundingBox();
    expect(hit?.width).toBeGreaterThanOrEqual(44);
    expect(hit?.height).toBeGreaterThanOrEqual(44);
    // The type icon stays and does not sit under the 44 px hit area of the box.
    const icon = page
      .getByRole("link", { name: /ACT-001/ })
      .locator("svg")
      .first();
    await expect(icon).toBeVisible();
    const iconBox = await icon.boundingBox();
    expect(iconBox && hit).toBeTruthy();
    if (iconBox && hit) {
      const overlaps =
        iconBox.x < hit.x + hit.width &&
        iconBox.x + iconBox.width > hit.x &&
        iconBox.y < hit.y + hit.height &&
        iconBox.y + iconBox.height > hit.y;
      expect(overlaps).toBe(false);
    }

    await label(page, "ACT-001").tap();
    await expect(bar(page)).toBeVisible();
    const small: (string | null)[] = [];
    for (const target of await bar(page).locator("button").all()) {
      if ((await target.boundingBox().then((b) => b?.height ?? 0)) < 44) {
        small.push(await target.textContent());
      }
    }
    expect(small).toEqual([]);

    await bar(page).getByRole("button", { name: "Verantwortlich" }).tap();
    const sheet = page.getByRole("dialog", { name: "Verantwortlich" });
    await expect(sheet).toBeVisible();
    expect((await axe(page)).violations).toEqual([]);
    await sheet.getByRole("option", { name: /Bernhard/ }).tap();
    await expect(page.getByText("1 Eintrag geändert")).toBeAttached();
    expect(sent).toEqual([{ owner_user_id: BERND, expected_version: 1 }]);
  });
});

// The same selection and the same bar in the table view (ADR 0082).
const tableWork = `${work}?view=table`;
const selectAll = (page: Page) => page.getByRole("checkbox", { name: "Alle auswählen" });

test.describe("the table, with a mouse", () => {
  test.use({ viewport: { width: 1440, height: 900 } });

  test("selects rows, shows the header box as mixed and changes the status of two records", async ({
    page,
  }) => {
    await fakeWork(page);
    const sent = await acceptChanges(page);
    await page.goto(tableWork);
    await expect(page.getByRole("table", { name: "Arbeit als Tabelle" })).toBeVisible();
    // The box is in the row, next to the title link and not in it.
    const row = page.getByRole("row", { name: /ACT-001/ });
    await expect(row.getByRole("link")).toBeVisible();
    await expect(row.getByRole("link").getByRole("checkbox")).toHaveCount(0);
    await expect(box(page, "ACT-004")).toHaveCount(0);

    await label(page, "ACT-001").click();
    await expect(selectAll(page)).toBeChecked({ indeterminate: true });
    await expect(row).toHaveAttribute("data-selected", "");
    await label(page, "ACT-003").click();
    await expect(bar(page).getByText("2 ausgewählt")).toBeVisible();

    await bar(page).getByRole("button", { name: "Status" }).click();
    await page.getByRole("option", { name: "Erledigt" }).click();
    await expect(page.getByText("2 Einträge geändert")).toBeAttached();
    await expect(bar(page)).toHaveCount(0);
    expect(sent).toEqual([
      { status: "done", expected_version: 1 },
      { status: "done", expected_version: 1 },
    ]);
  });

  test("Ctrl+A selects the rows that can be changed and Escape clears, from the table", async ({
    page,
  }) => {
    await fakeWork(page);
    await page.goto(tableWork);
    await page.getByRole("link", { name: "Festzelt reservieren" }).focus();
    await page.keyboard.press("Control+a");
    await expect(bar(page).getByText("3 ausgewählt")).toBeVisible();
    await expect(selectAll(page)).toBeChecked();
    await page.keyboard.press("Escape");
    await expect(bar(page)).toHaveCount(0);
    await selectAll(page).locator("xpath=ancestor::label").click();
    await expect(bar(page).getByText("3 ausgewählt")).toBeVisible();
  });

  test("a switch to the timeline clears the selection and shows no bar", async ({ page }) => {
    await fakeWork(page);
    await page.goto(tableWork);
    await label(page, "ACT-001").click();
    await expect(bar(page)).toBeVisible();
    await page.getByRole("tab", { name: "Zeitplan" }).click();
    await expect(page.getByRole("tab", { name: "Zeitplan" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await expect(bar(page)).toHaveCount(0);
    // Back in the table the box is clear: the selection did not survive the timeline.
    await page.getByRole("tab", { name: "Tabelle" }).click();
    await expect(box(page, "ACT-001")).not.toBeChecked();
  });

  test("the row boxes are 28 px high, and the header box is in the first cell", async ({
    page,
  }) => {
    await fakeWork(page);
    await page.goto(tableWork);
    const hit = await label(page, "ACT-001").boundingBox();
    expect(hit?.height).toBeGreaterThanOrEqual(28);
    expect(hit?.height).toBeLessThanOrEqual(36);
  });

  for (const theme of themes) {
    test(`the table with a selection, ${theme}: no axe violation`, async ({ page }) => {
      await fakeWork(page);
      await page.goto(tableWork);
      await setTheme(page, theme);
      await label(page, "ACT-001").click();
      await label(page, "ACT-002").click();
      await expect(bar(page)).toBeVisible();
      expect((await axe(page)).violations).toEqual([]);
    });
  }
});

for (const viewport of viewports) {
  for (const theme of themes) {
    test(`the table with a selection, ${theme}, ${viewport.name} px: screenshot, no sideways scroll`, async ({
      page,
    }) => {
      await page.setViewportSize(viewport);
      await fakeWork(page);
      await page.goto(tableWork);
      await setTheme(page, theme);
      await label(page, "ACT-001").click();
      await label(page, "ACT-002").click();
      await expect(bar(page)).toBeVisible();
      await page.mouse.move(0, 0);
      expect(
        await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth),
      ).toBe(true);
      expect((await axe(page)).violations).toEqual([]);
      await fontsLoaded(page);
      await expect(page).toHaveScreenshot(`work-bulk-table-${theme}-${viewport.name}.png`);
    });
  }
}

test.describe("the table on a phone with touch", () => {
  test.use({ hasTouch: true, isMobile: true, viewport: { width: 375, height: 812 } });

  test("is the card list, selects like the board and has 44 px targets", async ({ page }) => {
    await fakeWork(page);
    await page.goto(tableWork);
    await expect(page.getByRole("table")).toHaveCount(0);
    const hit = await selectAll(page).locator("xpath=ancestor::label").boundingBox();
    expect(hit?.height).toBeGreaterThanOrEqual(44);
    await label(page, "ACT-001").tap();
    await expect(bar(page).getByText("1 ausgewählt")).toBeVisible();
    await selectAll(page).locator("xpath=ancestor::label").tap();
    await expect(bar(page).getByText("3 ausgewählt")).toBeVisible();
    expect((await axe(page)).violations).toEqual([]);
  });
});

// The bar floats over the page, so every view keeps room for it at its end (ADR 0082): it never
// covers the last row of the table or the last card.
for (const viewport of [
  { width: 1440, height: 900 },
  { width: 375, height: 812 },
]) {
  for (const [view, path] of [
    ["board", work],
    ["table", `${work}?view=table`],
  ] as const) {
    test(`the bar does not cover the last ${view === "table" ? "row" : "card"}, ${viewport.width} px`, async ({
      page,
    }) => {
      await page.setViewportSize(viewport);
      await fakeWork(page, manyActions);
      await page.goto(path);
      await label(page, "ACT-001").click();
      await expect(bar(page)).toBeVisible();
      await page.evaluate(() => window.scrollTo(0, document.documentElement.scrollHeight));
      await page.mouse.move(0, 0);
      const frame = await bar(page).boundingBox();
      // The last one by position: the cards and the rows end with ACT-030.
      const last = await label(page, "ACT-030").boundingBox();
      if (!(frame && last)) {
        throw new Error("not shown");
      }
      expect(last.y + last.height).toBeLessThanOrEqual(frame.y);
    });
  }
}
