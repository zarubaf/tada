import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";
import { fontsLoaded, setTheme, themes, viewports } from "./fixtures";
import { event, fakeWork, fulfill } from "./workFixtures";

// Milestones and decisions in the side panel of the „Arbeit“ page (ADR 0082). The unit tests hold
// the rules; these checks hold what only a browser shows: the same rhythm as the action panel, the
// hints, the links, the dialog of „Ersetzen“ with its picker, and the screenshots.
const work = `/events/${event?.id}/work`;

const axe = (page: Page) =>
  new AxeBuilder({ page }).withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"]).analyze();

const ME = "0199b8e0-0000-7000-8000-0000000000b1";
const ACTION = "0199b8e0-0000-7000-8000-000000000101";

const milestones = [
  {
    id: "0199b8e0-0000-7000-8000-000000000121",
    local_id: "MIL-001",
    event_id: event?.id,
    title: "Stromversorgung für Festwirtschaft und Flugplatzinfrastruktur gesichert",
    description: "Der Generator steht und ist abgenommen.\nDie Zuleitung ist geprüft.",
    status: "open",
    date: "2020-05-15",
    owner_user_id: ME,
    workstream_id: "0199b8e0-0000-7000-8000-0000000000c1",
    warnings: [{ kind: "overdue" }, { kind: "late-action", action_id: ACTION }],
    next_statuses: ["reached", "canceled"],
    redacted: false,
    version: 1,
    can_change: true,
    evidence: [],
  },
];

const party = (kind: string, id: string, localId: string, name: string) => ({
  kind,
  id,
  local_id: localId,
  name,
  redacted: false,
});

const decisions = [
  {
    id: "0199b8e0-0000-7000-8000-000000000131",
    local_id: "DEC-001",
    event_id: event?.id,
    wording:
      "Das Fest findet bei jedem Wetter statt.\nBei Sturm entscheidet das OK am Vortag über eine Verschiebung.",
    status: "in-force",
    decided_on: "2030-03-12",
    decided_by: [
      party("person", "0199b8e0-0000-7000-8000-000000000a01", "PER-001", "Cäcilia Probst"),
      party(
        "institution",
        "0199b8e0-0000-7000-8000-000000000a02",
        "INS-001",
        "Testwil Generatoren AG",
      ),
    ],
    owner_user_id: ME,
    workstream_id: null,
    redacted: false,
    version: 2,
    can_change: true,
    can_end: true,
    evidence: [],
  },
];

/** Chooses an action of the record in the „Aktionen“ menu of the panel head. */
async function chooseAction(page: Page, id: string, action: string) {
  const panel = page.getByRole("complementary", { name: new RegExp(`^${id}`) });
  await panel.getByRole("button", { name: "Aktionen" }).click();
  await page.getByRole("menuitem", { name: action }).click();
}

async function open(page: Page, search: string) {
  await fakeWork(page, { milestones, decisions });
  await fulfill(page, "**/api/v1/persons*", {
    items: [
      { id: "0199b8e0-0000-7000-8000-000000000a01", local_id: "PER-001", name: "Cäcilia Probst" },
    ],
  });
  await fulfill(page, "**/api/v1/institutions*", {
    items: [
      {
        id: "0199b8e0-0000-7000-8000-000000000a02",
        local_id: "INS-001",
        name: "Testwil Generatoren AG",
      },
    ],
  });
  await page.clock.setFixedTime(new Date("2030-05-08T10:00:00Z"));
  await page.goto(`${work}${search}`);
  await expect(page.getByRole("heading", { level: 1, name: "Arbeit" })).toBeVisible();
}

for (const viewport of viewports) {
  for (const theme of themes) {
    const name = `${theme}, ${viewport.name} px`;

    test(`a milestone, ${name}: no axe violation, screenshot`, async ({ page }) => {
      await page.setViewportSize(viewport);
      await open(page, "?view=table#MIL-001");
      await setTheme(page, theme);
      const panel = page.getByRole("complementary", { name: /^MIL-001/ });
      await expect(panel.getByRole("region", { name: "Hinweise" })).toBeVisible();
      await expect(panel.getByText("ACT-001 ist erst danach fällig")).toBeVisible();
      await expect
        .poll(async () => (await panel.boundingBox())?.x)
        .toBe(viewport.width < 640 ? 0 : viewport.width - 480);
      expect((await axe(page)).violations).toEqual([]);
      await fontsLoaded(page);
      await expect(page).toHaveScreenshot(`panel-milestone-${theme}-${viewport.name}.png`);
    });

    test(`a decision, ${name}: no axe violation, screenshot`, async ({ page }) => {
      await page.setViewportSize(viewport);
      await open(page, "?view=table#DEC-001");
      await setTheme(page, theme);
      const panel = page.getByRole("complementary", { name: /^DEC-001/ });
      await expect(panel.getByRole("button", { name: "Aktionen" })).toBeVisible();
      await expect(panel.getByText("Testwil Generatoren AG")).toBeVisible();
      await expect
        .poll(async () => (await panel.boundingBox())?.x)
        .toBe(viewport.width < 640 ? 0 : viewport.width - 480);
      expect((await axe(page)).violations).toEqual([]);
      await fontsLoaded(page);
      await expect(page).toHaveScreenshot(`panel-decision-${theme}-${viewport.name}.png`);
    });

    test(`the dialog of Ersetzen, ${name}: no axe violation, screenshot`, async ({ page }) => {
      await page.setViewportSize(viewport);
      await open(page, "?view=table#DEC-001");
      await setTheme(page, theme);
      await chooseAction(page, "DEC-001", "Ersetzen");
      const dialog = page.getByRole("dialog", { name: "DEC-001 ersetzen" });
      await expect(dialog.getByRole("textbox", { name: /^Wortlaut/ })).toBeFocused();
      await expect(dialog.getByRole("option", { name: "Cäcilia Probst (PER-001)" })).toBeVisible();
      // The buttons stay on screen: the fields scroll, the dialog does not outgrow the viewport.
      const box = await dialog.boundingBox();
      expect((box?.y ?? 0) + (box?.height ?? 0)).toBeLessThanOrEqual(viewport.height);
      expect((await axe(page)).violations).toEqual([]);
      await fontsLoaded(page);
      await expect(page).toHaveScreenshot(`dialog-supersede-${theme}-${viewport.name}.png`);
    });
  }
}

// The step buttons keep their place from record to record: a manager who presses „Nächster Eintrag“
// twice in the same spot must not go forward and then back, whether the record offers actions or not.
test("the step buttons are in the same place for every kind of record", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await open(page, "?view=table");
  const where: Record<string, { previous: number; next: number }> = {};
  for (const id of ["ACT-001", "COM-001", "DEC-001", "MIL-001"]) {
    await page.goto(`${work}?view=table#${id}`);
    const panel = page.getByRole("complementary", { name: new RegExp(`^${id}`) });
    const previous = panel.getByRole("button", { name: "Vorheriger Eintrag" });
    const next = panel.getByRole("button", { name: "Nächster Eintrag" });
    await expect(next).toBeVisible();
    // The panel slides in; measure it when it has stopped.
    await expect.poll(async () => (await panel.boundingBox())?.x).toBe(960);
    where[id] = {
      previous: (await previous.boundingBox())?.x ?? -1,
      next: (await next.boundingBox())?.x ?? -1,
    };
  }
  const first = where["ACT-001"];
  expect(first?.next).toBeGreaterThan(0);
  for (const place of Object.values(where)) {
    expect(place).toEqual(first);
  }
});

const replaced = [
  {
    ...decisions[0],
    status: "superseded",
    superseded_by: "0199b8e0-0000-7000-8000-000000000132",
  },
  {
    ...decisions[0],
    id: "0199b8e0-0000-7000-8000-000000000132",
    local_id: "DEC-002",
    wording: "Das Fest findet nur bei Sonne statt.",
  },
];

test("the successor of a replaced decision opens in the panel, with focus and Back", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await fakeWork(page, { decisions: replaced });
  await page.goto(`${work}#DEC-001`);
  const panel = page.getByRole("complementary", { name: /^DEC-001/ });
  await panel.getByRole("link", { name: "DEC-002" }).click();
  const next = page.getByRole("complementary", { name: /^DEC-002/ });
  await expect(next).toBeVisible();
  expect(new URL(page.url()).hash).toBe("#DEC-002");
  // The link left with the old record: focus is on the title of the new one, not on the body.
  await expect(next.getByRole("heading", { level: 2 })).toBeFocused();
  await page.goBack();
  const back = page.getByRole("complementary", { name: /^DEC-001/ });
  await expect(back).toBeVisible();
  expect(new URL(page.url()).hash).toBe("#DEC-001");
  await expect(back.getByRole("heading", { level: 2 })).toBeFocused();
});

test("Ersetzen leaves focus on the title of the decision", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await open(page, "#DEC-001");
  await page.route("**/api/v1/events/*/decisions/*/supersede", (route) =>
    route.fulfill({
      status: 201,
      contentType: "application/json",
      body: JSON.stringify({
        ...decisions[0],
        id: "0199b8e0-0000-7000-8000-000000000132",
        local_id: "DEC-002",
        version: 1,
      }),
    }),
  );
  const panel = page.getByRole("complementary", { name: /^DEC-001/ });
  await chooseAction(page, "DEC-001", "Ersetzen");
  const dialog = page.getByRole("dialog", { name: "DEC-001 ersetzen" });
  await dialog.getByLabel("Entschieden (Pflichtfeld)").fill("2030-06-01");
  await dialog.getByRole("button", { name: "Ersetzen" }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(panel.getByRole("heading", { level: 2 })).toBeFocused();
});

test("the people of a decision keep the rhythm of a row, and an ID stays on one line", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await open(page, "#DEC-001");
  const panel = page.getByRole("complementary", { name: /^DEC-001/ });
  await expect(panel.getByText("Testwil Generatoren AG")).toBeVisible();
  const rows = await panel.locator("ul li").evaluateAll((items) =>
    items.map((item) => {
      const box = item.getBoundingClientRect();
      return { top: box.top, height: box.height };
    }),
  );
  expect(rows).toHaveLength(2);
  expect(rows[0]?.height).toBe(32);
  expect((rows[1]?.top ?? 0) - (rows[0]?.top ?? 0)).toBe(32);
  // The label is level with the first person.
  const label = await panel.getByText("Beteiligte", { exact: true }).boundingBox();
  expect(label?.y).toBeGreaterThanOrEqual((rows[0]?.top ?? 0) - 1);
  expect((label?.y ?? 0) + (label?.height ?? 0)).toBeLessThanOrEqual((rows[0]?.top ?? 0) + 32 + 1);

  await chooseAction(page, "DEC-001", "Ersetzen");
  const heading = page.getByRole("dialog", { name: "DEC-001 ersetzen" }).getByRole("heading");
  const lines = await heading
    .locator("span")
    .evaluate((id) => new Set(Array.from(id.getClientRects(), (rect) => rect.top)).size);
  expect(lines).toBe(1);
});

test.describe("with a coarse pointer", () => {
  test.use({ hasTouch: true, isMobile: true, viewport: { width: 390, height: 844 } });

  // ADR 0022: the target is 44 px.
  test("the link to the successor is 44 px high", async ({ page }) => {
    await fakeWork(page, { decisions: replaced });
    await page.goto(`${work}#DEC-001`);
    const link = page
      .getByRole("complementary", { name: /^DEC-001/ })
      .getByRole("link", { name: "DEC-002" });
    expect((await link.boundingBox())?.height).toBeGreaterThanOrEqual(44);
  });
});

test("Escape closes the dialog first and the panel second", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await open(page, "#DEC-001");
  const panel = page.getByRole("complementary", { name: /^DEC-001/ });
  await chooseAction(page, "DEC-001", "Widerrufen");
  await expect(page.getByRole("dialog", { name: "DEC-001 widerrufen" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(panel).toBeVisible();
  await expect(panel.getByRole("button", { name: "Aktionen" })).toBeFocused();
});
