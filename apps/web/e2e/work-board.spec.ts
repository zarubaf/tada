import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";
import {
  events,
  fakeEvent,
  fakeSession,
  fontsLoaded,
  problemBody,
  sessionInfo,
  sessionWithRole,
  setTheme,
  textOverflows,
  themes,
  viewports,
} from "./fixtures";

const event = events[0];
const ME = sessionInfo.user_id;
const BERND = "0199b8e0-0000-7000-8000-0000000000b2";
const CLARA = "0199b8e0-0000-7000-8000-0000000000b3";
const GROUND = "0199b8e0-0000-7000-8000-0000000000c1";
const FEST = "0199b8e0-0000-7000-8000-0000000000c2";

// Invented records with long German names and umlauts (doc/design/principles.md). The dates are far
// from today, so that the overdue chip does not depend on the day of the run.
const members = [
  { user_id: ME, display_name: "Anna Muster", role: "member", version: 1 },
  {
    user_id: BERND,
    display_name: "Bernhard Beispiel-Schmidlin-Äbischer",
    role: "member",
    version: 1,
  },
  { user_id: CLARA, display_name: "Cäcilia Probst", role: "member", version: 1 },
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
    lead_user_id: CLARA,
    status: "active",
    version: 1,
  },
  {
    id: FEST,
    event_id: event?.id,
    name: "Festwirtschaft",
    lead_user_id: BERND,
    status: "active",
    version: 1,
  },
];

function base(n: string, owner: string, over: Record<string, unknown>) {
  return {
    id: `0199b8e0-0000-7000-8000-0000000001${n}`,
    event_id: event?.id,
    owner_user_id: owner,
    redacted: false,
    version: 1,
    evidence: [],
    can_change: true,
    ...over,
  };
}

const actions = [
  base("01", ME, {
    local_id: "ACT-001",
    title: "Generatorenvermietung Testwil-Oberdorf anfragen und Angebote vergleichen",
    status: "open",
    due_date: "2020-05-18",
    workstream_id: GROUND,
    next_statuses: [],
  }),
  base("02", BERND, {
    local_id: "ACT-002",
    title: "Festzelt reservieren",
    status: "blocked",
    due_date: "2099-06-01",
    workstream_id: FEST,
    next_statuses: [],
  }),
  base("03", CLARA, {
    local_id: "ACT-003",
    title: "Flyer drucken",
    status: "in-progress",
    next_statuses: [],
  }),
  base("04", ME, {
    local_id: "ACT-004",
    title: "Hallenplan zeichnen",
    status: "done",
    due_date: "2020-01-10",
    next_statuses: [],
  }),
  base("05", ME, {
    local_id: "ACT-005",
    title: "Feuerwerk anfragen",
    status: "canceled",
    next_statuses: [],
  }),
];

const commitments = [
  base("11", BERND, {
    local_id: "COM-001",
    text: "Lieferung des Generators am Freitag um 15:00 Uhr auf das Flugplatzgelände",
    status: "conditional",
    due_date: "2099-05-17",
    workstream_id: GROUND,
    can_make_firm: false,
    next_statuses: [],
  }),
];

const milestones = [
  base("21", ME, {
    local_id: "MIL-001",
    title: "Stromversorgung für Festwirtschaft und Flugplatzinfrastruktur gesichert",
    status: "open",
    date: "2099-05-15",
    warnings: [],
    next_statuses: [],
  }),
];

const decisions = [
  base("31", ME, {
    local_id: "DEC-001",
    wording: "Das Fest findet bei jedem Wetter statt.",
    status: "in-force",
    decided_on: "2030-03-12",
    decided_by: [],
    can_end: false,
  }),
];

const risks = [
  base("41", CLARA, {
    local_id: "RSK-001",
    title: "Regen: kein Ausweichraum für die Turnshow",
    status: "occurred",
    likelihood: "high",
    impact: "high",
    next_statuses: [],
  }),
];

function fulfill(page: Page, pattern: string, body: unknown, status = 200) {
  return page.route(pattern, (route) =>
    route.fulfill({
      status,
      contentType: status < 400 ? "application/json" : "application/problem+json",
      body: JSON.stringify(body),
    }),
  );
}

async function fakeWork(page: Page, lists: Partial<Record<string, unknown[]>> = {}): Promise<void> {
  await fakeSession(page, sessionWithRole("member"));
  await fakeEvent(page, event);
  const all = { actions, commitments, milestones, decisions, risks, ...lists };
  await fulfill(page, "**/api/v1/events/*/memberships", { items: memberships });
  await fulfill(page, "**/api/v1/members*", { items: members });
  await fulfill(page, "**/api/v1/events/*/workstreams", { items: workstreams });
  for (const [kind, items] of Object.entries(all)) {
    await fulfill(page, `**/api/v1/events/*/${kind}*`, { items });
  }
}

const axe = (page: Page) =>
  new AxeBuilder({ page }).withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"]).analyze();

/** Waits for the board: the columns (wide) or the status picker (narrow). */
async function boardShown(page: Page, narrow: boolean) {
  if (narrow) {
    await expect(page.getByRole("button", { name: /Offen \(\d+\)/ })).toBeVisible();
  } else {
    await expect(page.getByRole("heading", { level: 2, name: /^Offen/ })).toBeVisible();
  }
  await expect(page.getByRole("status", { name: "Arbeit wird geladen" })).toHaveCount(0);
}

const work = `/events/${event?.id}/work`;

for (const viewport of viewports) {
  for (const theme of themes) {
    test(`board, ${theme}, ${viewport.name} px: no axe violation, screenshot`, async ({ page }) => {
      await page.setViewportSize(viewport);
      await fakeWork(page);
      await page.goto(work);
      await setTheme(page, theme);
      await expect(page.getByRole("heading", { level: 1, name: "Arbeit" })).toBeVisible();
      await boardShown(page, viewport.width < 640);

      expect((await axe(page)).violations).toEqual([]);
      await fontsLoaded(page);
      await expect(page).toHaveScreenshot(`work-board-${theme}-${viewport.name}.png`, {
        fullPage: true,
      });
    });
  }
}

for (const theme of themes) {
  test(`collapsed column opened, ${theme}: no axe violation`, async ({ page }) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    await fakeWork(page);
    await page.goto(work);
    await setTheme(page, theme);
    await page.getByRole("button", { name: /^Abgebrochen/ }).click();
    await expect(page.getByRole("heading", { level: 2, name: /^Abgebrochen/ })).toBeVisible();
    expect((await axe(page)).violations).toEqual([]);
  });

  test(`filter sheet on a phone, ${theme}: no axe violation, screenshot`, async ({ page }) => {
    await page.setViewportSize({ width: 375, height: 812 });
    await fakeWork(page);
    await page.goto(work);
    await setTheme(page, theme);
    await boardShown(page, true);
    await page.getByRole("button", { name: "Filter" }).click();
    await expect(page.getByRole("dialog")).toBeVisible();
    expect((await axe(page)).violations).toEqual([]);
    await fontsLoaded(page);
    await expect(page).toHaveScreenshot(`work-board-filter-sheet-${theme}-375.png`);
  });

  test(`a failed kind shows its banner, ${theme}: no axe violation`, async ({ page }) => {
    await fakeWork(page);
    await fulfill(page, "**/api/v1/events/*/risks*", problemBody("unavailable", 503), 503);
    await page.goto(work);
    await setTheme(page, theme);
    await expect(page.getByText("Risiken konnten nicht geladen werden.")).toBeVisible();
    await expect(page.getByText("ACT-001")).toBeVisible();
    expect((await axe(page)).violations).toEqual([]);
  });

  test(`empty event, ${theme}: no axe violation`, async ({ page }) => {
    await fakeWork(page, {
      actions: [],
      commitments: [],
      milestones: [],
      decisions: [],
      risks: [],
    });
    await page.goto(work);
    await setTheme(page, theme);
    await expect(page.getByText("Noch keine Arbeit in diesem Anlass")).toBeVisible();
    expect((await axe(page)).violations).toEqual([]);
  });
}

test("a URL opens the same filtered view after a reload", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await fakeWork(page);
  await page.goto(`${work}?type=action&q=Flyer`);
  await expect(page.getByRole("link", { name: /ACT-003/ })).toBeVisible();
  await expect(page.getByRole("link", { name: /ACT-001/ })).toHaveCount(0);
  await page.reload();
  await expect(page.getByRole("link", { name: /ACT-003/ })).toBeVisible();
  await expect(page.getByRole("searchbox", { name: "Arbeit durchsuchen" })).toHaveValue("Flyer");
  await expect(page.getByRole("radio", { name: /^Aufgaben/, checked: true })).toBeVisible();
});

test("a card is one link to the old page of its record", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await fakeWork(page);
  await page.goto(work);
  await expect(page.getByRole("link", { name: /ACT-001/ })).toHaveAttribute(
    "href",
    `/events/${event?.id}/actions#ACT-001`,
  );
});

// ADR 0024: no text overflows with the 40 % longer pseudo-locale, and a phone does not scroll
// sideways (ADR 0023).
for (const viewport of viewports) {
  test(`pseudo-locale, ${viewport.name} px: no text overflows`, async ({ page }) => {
    await page.setViewportSize(viewport);
    await fakeWork(page);
    await page.goto(`${work}?pseudo`);
    // The pseudo-locale changes the words, so the first card is the sign that the board shows.
    await expect(page.locator("a[href$='#ACT-001']")).toBeVisible();
    expect(await textOverflows(page)).toEqual([]);
  });
}

test("at 320 px the board does not scroll sideways", async ({ page }) => {
  await page.setViewportSize({ width: 320, height: 800 });
  await fakeWork(page);
  await page.goto(work);
  await boardShown(page, true);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(
    true,
  );
});
