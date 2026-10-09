import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";
import {
  events,
  fakeEvent,
  fakeSession,
  sessionInfo,
  sessionWithRole,
  setTheme,
  themes,
} from "./fixtures";

// The supplier loop of Slice 2a, as the lead of a workstream sees it (spec: "Demonstration of 2a").
// An AI client proposed a new institution and a conditional commitment. The fakes stand in for
// the server; the Rust test crates/tada/tests/supplier_loop.rs proves the server side.
const event = events[0];
const ME = sessionInfo.user_id;
const SOURCE = "0199b8e0-0000-7000-8000-0000000000e1";
const CHANGESET = "0199b8e0-0000-7000-8000-000000000c11";
const P_INSTITUTION = "0199b8e0-0000-7000-8000-0000000001e1";
const P_COMMITMENT = "0199b8e0-0000-7000-8000-0000000001e2";
const INSTITUTION = "0199b8e0-0000-7000-8000-0000000005e1";
const COMMITMENT = "0199b8e0-0000-7000-8000-0000000005e2";
const WORKSTREAM = "0199b8e0-0000-7000-8000-0000000000c1";
const author = { kind: "ai", id: "0199b8e0-0000-7000-8000-0000000000c9", channel: "api-token" };

const TEXT = "Lieferung des Generators am Freitag um 15:00 Uhr auf das Flugplatzgelände";
const CONDITION = "vorbehältlich der unterzeichneten Bestellung";
const SUPPLIER = "Generatorenvermietung Testwil-Oberdorf Aktiengesellschaft";
const REASON = "Die Bestellung ist am Montag unterzeichnet worden.";

const evidence = {
  source_version_id: SOURCE,
  passage: {
    start: 0,
    end: 40,
    quote: "Wir liefern am Freitag um 15 Uhr, sobald die Bestellung steht.",
  },
  excerpt: {
    before: "",
    quote: "Wir liefern am Freitag um 15 Uhr, sobald die Bestellung steht.",
    after: " Freundliche Grüsse aus Testwil-Oberdorf.",
  },
};

function proposal(id: string, extra: object) {
  return {
    id,
    depends_on: [],
    reason: "Das Angebot der Firma nennt Termin und Bedingung.",
    evidence: [evidence],
    status: "open",
    stale: false,
    overdue: false,
    routed_to_me: true,
    can_review: true,
    ...extra,
  };
}

const changeset = {
  id: CHANGESET,
  event_id: event?.id,
  author,
  source_version_id: SOURCE,
  created_at: "2028-03-07T09:00:00Z",
  proposals: [
    proposal(P_INSTITUTION, {
      operation: {
        kind: "create-institution",
        id: INSTITUTION,
        name: SUPPLIER,
        institution_kind: "company",
        email: null,
        phone: null,
      },
    }),
    proposal(P_COMMITMENT, {
      depends_on: [P_INSTITUTION],
      operation: {
        kind: "create-commitment",
        id: COMMITMENT,
        event_id: event?.id,
        text: TEXT,
        promisor: { institution: INSTITUTION },
        owner: ME,
        workstream: WORKSTREAM,
        due_date: "2099-05-17",
        condition: CONDITION,
      },
    }),
  ],
};

const listItem = {
  id: CHANGESET,
  event_id: event?.id,
  author,
  created_at: "2028-03-07T09:00:00Z",
  open_proposals: 2,
  stale: false,
};

const conditional = {
  id: COMMITMENT,
  local_id: "COM-001",
  event_id: event?.id,
  text: TEXT,
  condition: CONDITION,
  promisor: { kind: "institution", id: INSTITUTION, local_id: "INS-001", name: SUPPLIER },
  owner_user_id: ME,
  workstream_id: WORKSTREAM,
  due_date: "2099-05-17",
  status: "conditional",
  version: 1,
  can_change: true,
  can_make_firm: true,
  next_statuses: ["fulfilled", "broken", "withdrawn"],
  evidence: [
    {
      record_version: 1,
      proposal_id: P_COMMITMENT,
      source_version_id: SOURCE,
      captured_at: "2028-03-07T09:30:00Z",
      start_offset: 0,
      end_offset: 40,
      quote: evidence.passage.quote,
    },
  ],
};
const firm = {
  ...conditional,
  status: "firm",
  firm_reason: REASON,
  version: 2,
  can_make_firm: false,
};

function json(body: unknown) {
  return { status: 200, contentType: "application/json", body: JSON.stringify(body) };
}

interface Requests {
  applies: unknown[];
  firms: unknown[];
}

/** A fake server with the state of the loop: the commitment exists after the apply. */
async function fakeLoop(page: Page): Promise<Requests> {
  const requests: Requests = { applies: [], firms: [] };
  let current: typeof conditional | undefined;
  let open = true;
  await fakeSession(page, sessionWithRole("member"));
  await fakeEvent(page, event);
  await page.route("**/api/v1/events?*", (route) => route.fulfill(json({ items: events })));
  await page.route(/\/api\/v1\/changesets\?/, (route) =>
    route.fulfill(json({ items: open ? [listItem] : [] })),
  );
  await page.route(/\/api\/v1\/changesets\/[^/]+$/, (route) =>
    route.fulfill(
      json({
        ...changeset,
        proposals: changeset.proposals.map((entry) => ({
          ...entry,
          status: open ? "open" : "accepted",
        })),
      }),
    ),
  );
  await page.route(/\/api\/v1\/changesets\/[^/]+\/apply$/, (route) => {
    requests.applies.push(route.request().postDataJSON());
    open = false;
    current = conditional;
    return route.fulfill(
      json({
        proposals: [
          { id: P_INSTITUTION, status: "accepted" },
          { id: P_COMMITMENT, status: "accepted" },
        ],
        open_questions: [],
        documents: [],
      }),
    );
  });
  await page.route("**/api/v1/events/*/memberships", (route) => route.fulfill(json({ items: [] })));
  await page.route("**/api/v1/members*", (route) =>
    route.fulfill(
      json({ items: [{ user_id: ME, display_name: "Anna Muster", role: "member", version: 1 }] }),
    ),
  );
  await page.route("**/api/v1/events/*/workstreams", (route) =>
    route.fulfill(
      json({
        items: [
          {
            id: WORKSTREAM,
            event_id: event?.id,
            name: "Bodenbetrieb",
            lead_user_id: ME,
            status: "active",
            version: 1,
          },
        ],
      }),
    ),
  );
  await page.route("**/api/v1/events/*/actions*", (route) => route.fulfill(json({ items: [] })));
  await page.route("**/api/v1/persons*", (route) => route.fulfill(json({ items: [] })));
  await page.route("**/api/v1/institutions*", (route) => route.fulfill(json({ items: [] })));
  await page.route(/\/api\/v1\/events\/[^/]+\/commitments(\?.*)?$/, (route) =>
    route.fulfill(json({ items: current ? [current] : [] })),
  );
  await page.route(/\/commitments\/[^/]+\/firm$/, (route) => {
    requests.firms.push(route.request().postDataJSON());
    current = firm as typeof conditional;
    return route.fulfill(json(firm));
  });
  await page.route("**/api/v1/me/work", (route) =>
    route.fulfill(
      json({
        actions: [],
        commitments: current ? [{ ...current, event_key: event?.key }] : [],
        review_count: open ? 1 : 0,
      }),
    ),
  );
  return requests;
}

async function expectNoViolation(page: Page): Promise<void> {
  const results = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"])
    .analyze();
  expect(results.violations).toEqual([]);
}

for (const theme of themes) {
  test(`supplier loop, ${theme}: review, conditional, make firm, My Work`, async ({ page }) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    const requests = await fakeLoop(page);

    // The lead sees the changeset with the new institution and the conditional commitment.
    await page.goto(`/inbox/${CHANGESET}`);
    await setTheme(page, theme);
    await expect(page.getByRole("heading", { level: 1, name: "Eingang" })).toBeVisible();
    await expect(
      page.getByRole("article", { name: `Institution „${SUPPLIER}“ anlegen` }),
    ).toBeVisible();
    const card = page.getByRole("article", { name: "Zusage anlegen" });
    await expect(card).toBeVisible();
    await expect(card.getByText(CONDITION)).toBeVisible();
    await expectNoViolation(page);

    // Accepting the commitment takes the institution it depends on.
    await card.getByRole("button", { name: "Annehmen", exact: true }).click();
    await expect(page.getByText("2 Vorschläge angenommen.")).toBeVisible();
    expect(requests.applies).toEqual([
      { selected: expect.arrayContaining([P_INSTITUTION, P_COMMITMENT]), edits: [], links: [] },
    ]);

    // The register shows the commitment as conditional, with its condition.
    await page.goto(`/events/${event?.id}/commitments`);
    const register = page.getByRole("table", { name: "Zusagen" });
    const row = register.getByRole("row", { name: /COM-001/ });
    await expect(row.getByText("bedingt", { exact: true })).toBeVisible();
    await expect(row.getByText(`Bedingung: ${CONDITION}`)).toBeVisible();
    await expectNoViolation(page);

    // The lead makes it firm with a reason.
    await page.getByRole("button", { name: "COM-001 verbindlich machen" }).click();
    const dialog = page.getByRole("dialog", { name: "COM-001 verbindlich machen" });
    await expect(dialog).toBeVisible();
    await dialog.getByRole("textbox").fill(REASON);
    await expectNoViolation(page);
    await dialog.getByRole("button", { name: "Verbindlich machen" }).click();
    await expect(page.getByText("COM-001 ist jetzt verbindlich.")).toBeVisible();
    await expect(row.getByText("verbindlich", { exact: true })).toBeVisible();
    expect(requests.firms).toEqual([{ reason: REASON, expected_version: 1 }]);

    // My Work shows the commitment.
    await page.goto("/");
    await expect(page.getByRole("heading", { level: 1, name: "Meine Arbeit" })).toBeVisible();
    await expect(page.getByText(TEXT)).toBeVisible();
    await expect(page.getByText("verbindlich", { exact: true })).toBeVisible();
    await expectNoViolation(page);
  });
}
