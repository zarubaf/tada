import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";
import {
  events,
  fakeEvent,
  fakeProfile,
  fakeSession,
  fontsLoaded,
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

// Invented records with long German names and umlauts (doc/design/principles.md).
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
    id: "0199b8e0-0000-7000-8000-0000000000c1",
    event_id: event?.id,
    name: "Bodenbetrieb und Flugplatzinfrastruktur",
    lead_user_id: CLARA,
    status: "active",
    version: 1,
  },
  {
    id: "0199b8e0-0000-7000-8000-0000000000c2",
    event_id: event?.id,
    name: "Festwirtschaft",
    lead_user_id: BERND,
    status: "closed",
    version: 3,
  },
];

const actions = [
  {
    id: "0199b8e0-0000-7000-8000-0000000000d1",
    local_id: "ACT-001",
    event_id: event?.id,
    title: "Generatorenvermietung Testwil-Oberdorf anfragen und Angebote vergleichen",
    description: "Drei Angebote einholen.",
    owner_user_id: ME,
    workstream_id: workstreams[0]?.id,
    start_date: "2030-05-04",
    due_date: "2030-05-18",
    milestone_id: "0199b8e0-0000-7000-8000-000000000101",
    status: "in-progress",
    version: 2,
    can_change: true,
    next_statuses: ["open", "done"],
  },
  {
    id: "0199b8e0-0000-7000-8000-0000000000d2",
    local_id: "ACT-002",
    event_id: event?.id,
    title: "Festzelt reservieren",
    owner_user_id: BERND,
    status: "blocked",
    version: 1,
    can_change: true,
    next_statuses: ["open", "done"],
  },
  {
    id: "0199b8e0-0000-7000-8000-0000000000d3",
    local_id: "ACT-003",
    event_id: event?.id,
    title: "Flyer drucken",
    owner_user_id: CLARA,
    due_date: "2030-04-02",
    status: "done",
    version: 4,
    can_change: true,
    next_statuses: ["open", "done"],
  },
];

const commitments = [
  {
    id: "0199b8e0-0000-7000-8000-0000000000e1",
    local_id: "COM-001",
    event_id: event?.id,
    text: "Lieferung des Generators am Freitag um 15:00 Uhr auf das Flugplatzgelände",
    condition: "vorbehältlich der unterzeichneten Bestellung",
    promisor: {
      kind: "institution",
      id: "0199b8e0-0000-7000-8000-0000000000f1",
      local_id: "INS-001",
      name: "Generatorenvermietung Testwil-Oberdorf Aktiengesellschaft",
    },
    owner_user_id: ME,
    workstream_id: workstreams[0]?.id,
    due_date: "2030-05-17",
    status: "conditional",
    version: 3,
    can_change: true,
    can_make_firm: true,
    next_statuses: ["fulfilled", "broken", "withdrawn"],
    evidence: [
      {
        record_version: 1,
        proposal_id: "0199b8e0-0000-7000-8000-0000000000a9",
        source_version_id: "0199b8e0-0000-7000-8000-0000000000a8",
        captured_at: "2030-05-01T10:30:00Z",
        start_offset: 0,
        end_offset: 40,
        quote: "Wir liefern am Freitag um 15 Uhr, sobald die Bestellung unterzeichnet ist.",
        page: 2,
      },
    ],
  },
  {
    id: "0199b8e0-0000-7000-8000-0000000000e2",
    local_id: "COM-002",
    event_id: event?.id,
    text: "Bewilligung für den Flugbetrieb",
    promisor: {
      kind: "institution",
      id: "0199b8e0-0000-7000-8000-0000000000f2",
      local_id: "INS-002",
      name: "Gemeinde Testwil",
    },
    owner_user_id: BERND,
    status: "firm",
    firm_reason: "Bewilligung liegt vor.",
    version: 2,
    can_change: true,
    can_make_firm: false,
    next_statuses: ["fulfilled", "broken", "withdrawn"],
    evidence: [],
  },
];

const milestones = [
  {
    id: "0199b8e0-0000-7000-8000-000000000101",
    local_id: "MIL-001",
    event_id: event?.id,
    title: "Stromversorgung für Festwirtschaft und Flugplatzinfrastruktur gesichert",
    redacted: false,
    date: "2030-05-15",
    owner_user_id: ME,
    workstream_id: workstreams[0]?.id,
    status: "open",
    warnings: [
      { kind: "overdue" },
      { kind: "late-action", action_id: "0199b8e0-0000-7000-8000-0000000000d1" },
    ],
    version: 2,
    evidence: [],
    can_change: true,
    next_statuses: ["reached", "canceled"],
  },
  {
    id: "0199b8e0-0000-7000-8000-000000000102",
    local_id: "MIL-002",
    event_id: event?.id,
    title: "Bewilligung liegt vor",
    redacted: false,
    date: "2030-04-01",
    owner_user_id: BERND,
    status: "reached",
    warnings: [],
    version: 3,
    evidence: [],
    can_change: true,
    next_statuses: ["open"],
  },
];

const party = {
  kind: "institution",
  id: "0199b8e0-0000-7000-8000-0000000000f2",
  local_id: "INS-002",
  name: "Gemeinde Testwil",
  redacted: false,
};

const decisions = [
  {
    id: "0199b8e0-0000-7000-8000-000000000301",
    local_id: "DEC-001",
    event_id: event?.id,
    wording:
      "Das Fest findet bei jedem Wetter statt.\nBei Sturm entscheidet das Organisationskomitee am Vortag über eine Verschiebung auf den Ersatztermin.",
    redacted: false,
    decided_on: "2030-03-12",
    decided_by: [party],
    owner_user_id: ME,
    workstream_id: workstreams[0]?.id,
    status: "in-force",
    version: 2,
    evidence: [],
    can_change: true,
    can_end: true,
  },
  {
    id: "0199b8e0-0000-7000-8000-000000000302",
    local_id: "DEC-002",
    event_id: event?.id,
    wording: "Es gibt ein Feuerwerk am Samstagabend.",
    redacted: false,
    decided_on: "2030-02-01",
    decided_by: [party],
    owner_user_id: BERND,
    status: "revoked",
    revoked_reason: "Die Gemeinde verbietet das Feuerwerk.",
    version: 3,
    evidence: [],
    can_change: false,
    can_end: false,
  },
  {
    id: "0199b8e0-0000-7000-8000-000000000303",
    local_id: "DEC-003",
    event_id: event?.id,
    wording: "Das Fest dauert zwei Tage.",
    redacted: false,
    decided_on: "2030-01-15",
    decided_by: [party],
    owner_user_id: ME,
    status: "superseded",
    superseded_by: "0199b8e0-0000-7000-8000-000000000301",
    version: 3,
    evidence: [],
    can_change: false,
    can_end: false,
  },
];

function fulfill(page: Page, pattern: string, body: unknown) {
  return page.route(pattern, (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(body) }),
  );
}

async function fakeWork(page: Page): Promise<void> {
  await fakeEvent(page, event);
  await fulfill(page, "**/api/v1/events/*/memberships", { items: memberships });
  await fulfill(page, "**/api/v1/members*", { items: members });
  await fulfill(page, "**/api/v1/events/*/workstreams", { items: workstreams });
  await fulfill(page, "**/api/v1/events/*/actions*", { items: actions });
  await fulfill(page, "**/api/v1/events/*/commitments*", { items: commitments });
  await fulfill(page, "**/api/v1/events/*/milestones*", { items: milestones });
  await fulfill(page, "**/api/v1/events/*/decisions*", { items: decisions });
  await fulfill(page, "**/api/v1/persons*", { items: [] });
  await fulfill(page, "**/api/v1/institutions*", { items: [] });
}

const pages = [
  { name: "actions", path: "actions", title: "Aufgaben", table: "Aufgaben" },
  { name: "commitments", path: "commitments", title: "Zusagen", table: "Zusagen" },
  { name: "milestones", path: "milestones", title: "Meilensteine", table: "Meilensteine" },
  { name: "decisions", path: "decisions", title: "Entscheide", table: "Entscheide" },
  { name: "workstreams", path: "workstreams", title: "Arbeitsbereiche", table: "Arbeitsbereiche" },
] as const;

for (const viewport of viewports) {
  for (const theme of themes) {
    for (const shown of pages) {
      test(`${shown.name}, ${theme}, ${viewport.name} px: no axe violation, screenshot`, async ({
        page,
      }) => {
        await page.setViewportSize(viewport);
        await fakeSession(page, sessionWithRole("member"));
        await fakeWork(page);
        await page.goto(`/events/${event?.id}/${shown.path}`);
        await setTheme(page, theme);
        await expect(page.getByRole("heading", { level: 1, name: event?.name })).toBeVisible();
        await expect(page.getByRole("table", { name: shown.table })).toBeVisible();
        await expect(page.getByRole("button", { name: "Erfassen" })).toBeVisible();

        const results = await new AxeBuilder({ page })
          .withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"])
          .analyze();
        expect(results.violations).toEqual([]);

        await fontsLoaded(page);
        await expect(page).toHaveScreenshot(`${shown.name}-${theme}-${viewport.name}.png`, {
          fullPage: true,
        });
      });
    }
  }
}

for (const theme of themes) {
  test(`make firm dialog and evidence sheet, ${theme}: no axe violation`, async ({ page }) => {
    await fakeSession(page, sessionWithRole("member"));
    await fakeWork(page);
    await page.goto(`/events/${event?.id}/commitments`);
    await setTheme(page, theme);
    const axe = () =>
      new AxeBuilder({ page }).withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"]).analyze();

    await page.getByRole("button", { name: "COM-001 verbindlich machen" }).click();
    await expect(page.getByRole("dialog", { name: "COM-001 verbindlich machen" })).toBeVisible();
    expect((await axe()).violations).toEqual([]);
    await page.getByRole("button", { name: "Abbrechen" }).click();

    await page.getByRole("button", { name: "Beleg zu COM-001" }).click();
    await expect(page.getByRole("dialog", { name: "Beleg zu COM-001" })).toBeVisible();
    expect((await axe()).violations).toEqual([]);
  });
}

for (const theme of themes) {
  test(`revoke dialog, ${theme}: no axe violation`, async ({ page }) => {
    await fakeSession(page, sessionWithRole("member"));
    await fakeWork(page);
    await page.goto(`/events/${event?.id}/decisions`);
    await setTheme(page, theme);

    await page.getByRole("button", { name: "DEC-001 widerrufen" }).click();
    await expect(page.getByRole("dialog", { name: "DEC-001 widerrufen" })).toBeVisible();
    const results = await new AxeBuilder({ page })
      .withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"])
      .analyze();
    expect(results.violations).toEqual([]);
  });
}

// ADR 0024: no text overflows with the 40 % longer pseudo-locale, and the sub-navigation with seven
// links wraps instead of scrolling sideways.
for (const viewport of viewports) {
  for (const shown of pages) {
    test(`${shown.name}, pseudo-locale, ${viewport.name} px: no text overflows`, async ({
      page,
    }) => {
      await page.setViewportSize(viewport);
      await fakeSession(page, sessionWithRole("member"));
      await fakeWork(page);
      await page.goto(`/events/${event?.id}/${shown.path}?pseudo`);
      await expect(page.getByRole("table")).toBeVisible();
      await expect(page.locator("[role=status][aria-label]")).toHaveCount(0);

      expect(await textOverflows(page)).toEqual([]);
    });
  }
}

test("at 320 px the sub-navigation and the register do not scroll sideways", async ({ page }) => {
  await page.setViewportSize({ width: 320, height: 800 });
  await fakeSession(page, sessionWithRole("member"));
  await fakeWork(page);
  await page.goto(`/events/${event?.id}/commitments`);
  await expect(page.getByRole("table", { name: "Zusagen" })).toBeVisible();

  const nav = page.getByRole("navigation", { name: "Anlass" });
  await expect(nav.getByRole("link")).toHaveCount(7);
  await expect(page.getByRole("navigation", { name: "Register" }).getByRole("link")).toHaveCount(3);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(
    true,
  );
  expect(await textOverflows(page)).toEqual([]);
});

test("a manager records, replaces and revokes a decision; a contributor has no write action", async ({
  page,
}) => {
  await fakeSession(page, sessionWithRole("member"));
  await fakeWork(page);
  const shown: Record<string, unknown>[] = [];
  const base = {
    event_id: event?.id,
    redacted: false,
    decided_by: [party],
    owner_user_id: ME,
    version: 1,
    evidence: [],
    can_change: true,
    can_end: true,
  };
  const bodies: unknown[] = [];
  await page.route(/\/api\/v1\/events\/[^/]+\/decisions/, async (route) => {
    const request = route.request();
    const path = new URL(request.url()).pathname;
    const reply = (status: number, body: unknown) =>
      route.fulfill({ status, contentType: "application/json", body: JSON.stringify(body) });
    if (request.method() === "GET") {
      return reply(200, { items: shown });
    }
    const body = request.postDataJSON();
    bodies.push({ path, body });
    if (path.endsWith("/supersede")) {
      const next = {
        ...base,
        id: "0199b8e0-0000-7000-8000-000000000402",
        local_id: "DEC-002",
        wording: body.successor.wording,
        decided_on: body.successor.decided_on,
      };
      Object.assign(shown[0] ?? {}, {
        status: "superseded",
        superseded_by: next.id,
        can_end: false,
        version: 2,
      });
      shown.push({ ...next, status: "in-force" });
      return reply(201, next);
    }
    if (path.endsWith("/revoke")) {
      Object.assign(shown[1] ?? {}, {
        status: "revoked",
        revoked_reason: body.reason,
        can_end: false,
        version: 2,
      });
      return reply(200, shown[1]);
    }
    const created = {
      ...base,
      id: "0199b8e0-0000-7000-8000-000000000401",
      local_id: "DEC-001",
      wording: body.wording,
      decided_on: body.decided_on,
      status: "in-force",
    };
    shown.push(created);
    return reply(201, created);
  });
  await fulfill(page, "**/api/v1/institutions*", { items: [party] });

  await page.goto(`/events/${event?.id}/decisions`);
  await expect(page.getByText("Noch keine Entscheide")).toBeVisible();
  await page
    .getByRole("textbox", { name: "Wortlaut (Pflichtfeld)" })
    .fill("Zeile eins\nZeile zwei");
  await page.getByLabel("Beschlossen am (Pflichtfeld)").fill("2030-03-12");
  await page.getByRole("combobox", { name: "Beschlossen von (Pflichtfeld)" }).fill("Gemeinde");
  await page.getByRole("option", { name: "Gemeinde Testwil (INS-002)" }).click();
  await page.getByRole("button", { name: "Erfassen" }).click();
  const table = page.getByRole("table", { name: "Entscheide" });
  await expect(table.getByText("DEC-001")).toBeVisible();

  await page.getByRole("button", { name: "DEC-001 ersetzen" }).click();
  await expect(page.getByRole("heading", { name: "DEC-001 ersetzen" })).toBeFocused();
  await expect(page.getByRole("textbox", { name: "Wortlaut (Pflichtfeld)" })).toHaveValue(
    "Zeile eins\nZeile zwei",
  );
  await page.getByRole("textbox", { name: "Wortlaut (Pflichtfeld)" }).fill("Neue Zeile");
  await page.getByLabel("Beschlossen am (Pflichtfeld)").fill("2030-04-01");
  await page.getByRole("button", { name: "Ersetzen", exact: true }).click();
  const first = table.getByText("DEC-001").locator("xpath=ancestor::tr");
  await expect(first.getByText("ersetzt", { exact: true })).toBeVisible();
  await expect(first.getByRole("link", { name: "DEC-002" })).toBeVisible();

  await page.getByRole("button", { name: "DEC-002 widerrufen" }).click();
  const dialog = page.getByRole("dialog", { name: "DEC-002 widerrufen" });
  await dialog.getByRole("button", { name: "Widerrufen" }).click();
  await expect(dialog.getByText("Geben Sie einen Grund an.")).toBeVisible();
  await dialog.getByRole("textbox", { name: "Grund (Pflichtfeld)" }).fill("Plan geändert");
  await dialog.getByRole("button", { name: "Widerrufen" }).click();
  await expect(dialog).toBeHidden();
  const second = table.getByText("DEC-002").locator("xpath=ancestor::tr");
  await expect(second.getByText("widerrufen", { exact: true })).toBeVisible();
  await expect(second.getByText("Grund: Plan geändert")).toBeVisible();
  await expect(page.getByRole("heading", { level: 2, name: "Entscheide" })).toBeFocused();

  expect(bodies).toMatchObject([
    {
      path: expect.stringMatching(/\/decisions$/),
      body: {
        wording: "Zeile eins\nZeile zwei",
        decided_by: [{ kind: "institution", id: party.id }],
      },
    },
    {
      path: expect.stringMatching(/\/supersede$/),
      body: { expected_version: 1, successor: { wording: "Neue Zeile" } },
    },
    {
      path: expect.stringMatching(/\/revoke$/),
      body: { reason: "Plan geändert", expected_version: 1 },
    },
  ]);
});

test("a contributor sees the decisions and no write action", async ({ page }) => {
  await fakeSession(page, sessionWithRole("member"));
  await fakeWork(page);
  await fulfill(page, "**/api/v1/events/*/memberships", {
    items: memberships.map((membership, index) =>
      index === 0 ? { ...membership, event_role: "event-contributor" } : membership,
    ),
  });
  await fulfill(page, "**/api/v1/events/*/decisions*", {
    items: decisions.map((decision) => ({ ...decision, can_change: false, can_end: false })),
  });

  await page.goto(`/events/${event?.id}/decisions`);
  await expect(page.getByRole("table", { name: "Entscheide" })).toBeVisible();
  await expect(page.getByRole("button", { name: /ersetzen|widerrufen|Erfassen/ })).toHaveCount(0);
  await expect(page.getByRole("textbox", { name: /Wortlaut/ })).toHaveCount(0);
});

test("a member creates a milestone, links an action and sees the late action", async ({ page }) => {
  await fakeSession(page, sessionWithRole("member"));
  await fakeWork(page);
  const created = {
    ...milestones[1],
    id: "0199b8e0-0000-7000-8000-000000000103",
    local_id: "MIL-003",
    title: "Festzelt steht",
    date: "2030-05-20",
    owner_user_id: ME,
    status: "open",
    version: 1,
    next_statuses: ["reached", "canceled"],
  };
  const action = {
    ...actions[1],
    id: "0199b8e0-0000-7000-8000-0000000000d4",
    local_id: "ACT-004",
    title: "Zeltboden verlegen",
    owner_user_id: ME,
    due_date: "2030-05-25",
    milestone_id: created.id,
    status: "open",
  };
  const shown: unknown[] = [];
  const listed: unknown[] = [];
  let linked = false;
  let sent: unknown;
  await page.route("**/api/v1/events/*/milestones*", (route) =>
    route.request().method() === "POST"
      ? route.fulfill({
          status: 201,
          contentType: "application/json",
          body: JSON.stringify(created),
        })
      : route.fulfill({
          contentType: "application/json",
          body: JSON.stringify({
            items: linked
              ? [{ ...created, warnings: [{ kind: "late-action", action_id: action.id }] }]
              : shown,
          }),
        }),
  );
  await page.route("**/api/v1/events/*/actions*", (route) => {
    if (route.request().method() === "POST") {
      linked = true;
      listed.push(action);
      sent = route.request().postDataJSON();
      return route.fulfill({
        status: 201,
        contentType: "application/json",
        body: JSON.stringify(action),
      });
    }
    return route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({ items: listed }),
    });
  });

  await page.goto(`/events/${event?.id}/commitments`);
  await page
    .getByRole("navigation", { name: "Register" })
    .getByRole("link", { name: "Meilensteine" })
    .click();
  await expect(page.getByText("Noch keine Meilensteine")).toBeVisible();
  await page.getByRole("textbox", { name: "Titel (Pflichtfeld)" }).fill("Festzelt steht");
  await page.getByLabel("Datum (Pflichtfeld)").fill("2030-05-20");
  await page.getByRole("button", { name: "Erfassen" }).click();
  await expect(
    page.getByRole("table", { name: "Meilensteine" }).getByText("MIL-003"),
  ).toBeVisible();
  shown.push(created);

  await page
    .getByRole("navigation", { name: "Anlass" })
    .getByRole("link", { name: "Aufgaben" })
    .click();
  await page.getByRole("textbox", { name: "Titel (Pflichtfeld)" }).fill("Zeltboden verlegen");
  await page.getByLabel("Fällig am").fill("2030-05-25");
  await page.getByRole("combobox", { name: "Meilenstein" }).click();
  await page.getByRole("option", { name: "MIL-003 Festzelt steht" }).click();
  await page.getByRole("button", { name: "Erfassen" }).click();
  await expect(page.getByRole("table", { name: "Aufgaben" }).getByText("ACT-004")).toBeVisible();
  expect(sent).toMatchObject({ milestone_id: created.id, due_date: "2030-05-25" });

  await page
    .getByRole("navigation", { name: "Anlass" })
    .getByRole("link", { name: "Register" })
    .click();
  await page
    .getByRole("navigation", { name: "Register" })
    .getByRole("link", { name: "Meilensteine" })
    .click();
  const table = page.getByRole("table", { name: "Meilensteine" });
  await expect(table.getByText("ACT-004 ist erst danach fällig")).toBeVisible();
});

// „Zeitplan“: the dated actions in lanes, the milestones and the event window (an assumption).
const windowField = {
  id: "0199b8e0-0000-7000-8000-000000000201",
  key: "date_window",
  label: { kind: "message", id: "field-date_window" },
  value_type: { type: "date-window", granularity: null },
  value_schema: {},
  description: "",
  module: "core",
  status: "active",
};

const windowProfile = {
  facts: [
    {
      id: "0199b8e0-0000-7000-8000-000000000202",
      field_id: windowField.id,
      field_key: "date_window",
      state: "assumption",
      redacted: false,
      value: { type: "date-window", start: "2030-04-15", end: "2030-06-15", granularity: "day" },
      version: 1,
      evidence: [],
      accepted_by: { kind: "member", id: ME, channel: "web" },
      accepted_at: "2030-03-04T08:30:00Z",
    },
  ],
  proposals: [],
  open_questions: [],
};

async function fakeTimeline(page: Page): Promise<void> {
  await fakeWork(page);
  await fakeProfile(page, windowProfile, [windowField]);
}

for (const viewport of viewports) {
  for (const theme of themes) {
    test(`timeline, ${theme}, ${viewport.name} px: no axe violation, screenshot`, async ({
      page,
    }) => {
      await page.setViewportSize(viewport);
      await fakeSession(page, sessionWithRole("member"));
      await fakeTimeline(page);
      // The baselines must not depend on the real date: today is a day on the axis.
      await page.clock.setFixedTime(new Date("2030-05-08T10:00:00Z"));
      await page.goto(`/events/${event?.id}/timeline`);
      await setTheme(page, theme);
      await expect(page.getByRole("heading", { level: 2, name: "Zeitplan" })).toBeVisible();
      await expect(page.getByRole("list", { name: "Legende" }).getByText("Annahme")).toBeVisible();
      // The page never scrolls sideways, whatever the width of the axis.
      expect(
        await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth),
      ).toBe(true);

      const results = await new AxeBuilder({ page })
        .withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"])
        .analyze();
      expect(results.violations).toEqual([]);

      await fontsLoaded(page);
      await expect(page).toHaveScreenshot(`timeline-${theme}-${viewport.name}.png`, {
        fullPage: true,
      });
    });
  }
}

test("a span of years scrolls inside the timeline and never the page", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await fakeSession(page, sessionWithRole("member"));
  await fakeWork(page);
  await fulfill(page, "**/api/v1/events/*/actions*", {
    items: [{ ...actions[0], start_date: "2030-05-06", due_date: "2033-05-01" }],
  });
  await page.goto(`/events/${event?.id}/timeline`);
  const axis = page.getByRole("region", { name: "Zeitachse" });
  await expect(axis).toBeVisible();

  const sizes = await axis.evaluate((node) => ({
    inner: node.scrollWidth,
    outer: node.clientWidth,
  }));
  expect(sizes.inner).toBeGreaterThan(sizes.outer);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(
    true,
  );
});

test("at 320 px the timeline shows the list by week without a sideways scroll", async ({
  page,
}) => {
  await page.setViewportSize({ width: 320, height: 800 });
  await fakeSession(page, sessionWithRole("member"));
  await fakeTimeline(page);
  await page.clock.setFixedTime(new Date("2030-05-08T10:00:00Z"));
  await page.goto(`/events/${event?.id}/timeline`);
  await expect(page.getByRole("region", { name: "Nach Wochen" })).toBeVisible();
  await expect(page.getByRole("region", { name: "Zeitachse" })).toBeHidden();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(
    true,
  );
  expect(await textOverflows(page)).toEqual([]);
});

test("wide: the axis takes the keyboard and the list stays in the accessibility tree", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await fakeSession(page, sessionWithRole("member"));
  await fakeTimeline(page);
  await page.clock.setFixedTime(new Date("2030-05-08T10:00:00Z"));
  await page.goto(`/events/${event?.id}/timeline`);
  const axis = page.getByRole("region", { name: "Zeitachse" });
  await expect(axis).toBeVisible();

  // The list is out of sight, but a screen reader finds it by its role and name.
  const list = page.getByRole("region", { name: "Nach Wochen" });
  await expect(list).toHaveCount(1);
  await expect(list.getByRole("link", { name: /ACT-001/ })).toHaveAttribute("tabindex", "-1");
  await expect(axis.getByRole("link", { name: /ACT-001/ })).not.toHaveAttribute("tabindex", "-1");

  // Tab from the axis goes through its links only: no focused link lies in the hidden list.
  await axis.focus();
  for (let step = 0; step < 4; step += 1) {
    await page.keyboard.press("Tab");
    expect(
      await page.evaluate(() => document.activeElement?.closest("section")?.ariaLabel),
    ).not.toBe("Nach Wochen");
  }
});

test("narrow: the axis leaves the keyboard and the accessibility tree", async ({ page }) => {
  await page.setViewportSize({ width: 375, height: 800 });
  await fakeSession(page, sessionWithRole("member"));
  await fakeTimeline(page);
  await page.goto(`/events/${event?.id}/timeline`);
  const list = page.getByRole("region", { name: "Nach Wochen" });
  await expect(list).toBeVisible();
  await expect(page.getByRole("region", { name: "Zeitachse" })).toHaveCount(0);
  await expect(list.getByRole("link", { name: /ACT-001/ })).not.toHaveAttribute("tabindex", "-1");
});

test("a bar opens its row in the register of actions", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await fakeSession(page, sessionWithRole("member"));
  await fakeTimeline(page);
  await page.goto(`/events/${event?.id}/timeline`);
  await page
    .getByRole("region", { name: "Zeitachse" })
    .getByRole("link", { name: /ACT-001/ })
    .click();
  await expect(page).toHaveURL(/\/actions#ACT-001$/);
  await expect(page.locator("tr#ACT-001")).toBeFocused();
});

for (const contrast of ["no-preference", "more"] as const) {
  test(`hovering a milestone diamond changes its border, whatever its status (contrast ${contrast})`, async ({
    page,
  }) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.emulateMedia({ contrast });
    await fakeSession(page, sessionWithRole("member"));
    await fakeTimeline(page);
    const canceled = {
      ...milestones[1],
      id: "0199b8e0-0000-7000-8000-000000000109",
      local_id: "MIL-009",
      title: "Flugtag abgesagt",
      date: "2030-04-20",
      status: "canceled",
      next_statuses: ["open"],
    };
    await fulfill(page, "**/api/v1/events/*/milestones*", { items: [...milestones, canceled] });
    await page.goto(`/events/${event?.id}/timeline`);
    const axis = page.getByRole("region", { name: "Zeitachse" });
    // Open, reached and canceled. A screenshot cannot show a hover, so read the styles.
    for (const id of ["MIL-001", "MIL-002", "MIL-009"]) {
      const link = axis.getByRole("link", { name: new RegExp(id) });
      const diamond = link.locator("[data-status]");
      const border = () =>
        diamond.evaluate((el) => {
          const style = getComputedStyle(el);
          return `${style.borderTopColor} ${style.borderTopWidth}`;
        });
      await page.mouse.move(0, 0);
      const rest = await border();
      await link.hover();
      await expect.poll(border).not.toBe(rest);
    }
  });
}
