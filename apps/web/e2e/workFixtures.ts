// Invented work records for the browser checks of the „Arbeit“ page: five kinds, long German names
// and a fake API for them. The board and the views of the page share them.
import type { Page } from "@playwright/test";
import { events, fakeEvent, fakeSession, sessionInfo, sessionWithRole } from "./fixtures";

export const event = events[0];
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

export const actions = [
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

export const commitments = [
  base("11", BERND, {
    local_id: "COM-001",
    text: "Lieferung des Generators am Freitag um 15:00 Uhr auf das Flugplatzgelände",
    promisor: {
      kind: "institution",
      id: "0199b8e0-0000-7000-8000-0000000000d1",
      local_id: "INS-001",
      name: "Generatorenvermietung Testwil-Oberdorf AG",
      redacted: false,
    },
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

export const risks = [
  base("41", CLARA, {
    local_id: "RSK-001",
    title: "Regen: kein Ausweichraum für die Turnshow",
    status: "occurred",
    likelihood: "high",
    impact: "high",
    next_statuses: ["closed"],
  }),
];

export function fulfill(page: Page, pattern: string, body: unknown, status = 200) {
  return page.route(pattern, (route) =>
    route.fulfill({
      status,
      contentType: status < 400 ? "application/json" : "application/problem+json",
      body: JSON.stringify(body),
    }),
  );
}

export async function fakeWork(
  page: Page,
  lists: Partial<Record<string, unknown[]>> = {},
): Promise<void> {
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
