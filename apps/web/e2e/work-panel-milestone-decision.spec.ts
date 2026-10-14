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
      await expect(panel.getByRole("button", { name: "Ersetzen" })).toBeVisible();
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
      await page
        .getByRole("complementary", { name: /^DEC-001/ })
        .getByRole("button", { name: "Ersetzen" })
        .click();
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

test("the successor of a replaced decision opens in the panel", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
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
  await fakeWork(page, { decisions: replaced });
  await page.goto(`${work}#DEC-001`);
  const panel = page.getByRole("complementary", { name: /^DEC-001/ });
  await panel.getByRole("link", { name: "DEC-002" }).click();
  await expect(page.getByRole("complementary", { name: /^DEC-002/ })).toBeVisible();
  expect(new URL(page.url()).hash).toBe("#DEC-002");
});

test("Escape closes the dialog first and the panel second", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await open(page, "#DEC-001");
  const panel = page.getByRole("complementary", { name: /^DEC-001/ });
  await panel.getByRole("button", { name: "Widerrufen" }).click();
  await expect(page.getByRole("dialog", { name: "DEC-001 widerrufen" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(panel).toBeVisible();
  await expect(panel.getByRole("button", { name: "Widerrufen" })).toBeFocused();
});
