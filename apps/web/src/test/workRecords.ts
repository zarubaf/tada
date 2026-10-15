// Invented work records of the five kinds, in the shape of the API, for the tests of the „Arbeit“
// page. Each builder takes the readable ID and the fields that a test changes.
import { BERND, CLARA, EVENT, ME } from "./fakeWorkServer";

export const STREAM_GROUND = "w1";

type Over = Record<string, unknown>;

function base(localId: string, over: Over) {
  return {
    id: `rec-${localId.toLowerCase()}`,
    local_id: localId,
    event_id: EVENT.id,
    owner_user_id: ME,
    workstream_id: null,
    redacted: false,
    version: 1,
    evidence: [],
    can_change: true,
    ...over,
  };
}

export const action = (localId: string, over: Over = {}) =>
  base(localId, {
    title: `Aufgabe ${localId}`,
    status: "open",
    due_date: null,
    next_statuses: [],
    ...over,
  });

export const commitment = (localId: string, over: Over = {}) =>
  base(localId, {
    text: `Zusage ${localId}`,
    promisor: {
      kind: "institution",
      id: "rec-ins-001",
      local_id: "INS-001",
      name: "Bäckerei Zopf AG",
      redacted: false,
    },
    status: "conditional",
    due_date: null,
    can_make_firm: false,
    next_statuses: [],
    ...over,
  });

export const milestone = (localId: string, over: Over = {}) =>
  base(localId, {
    title: `Meilenstein ${localId}`,
    status: "open",
    date: "2031-01-01",
    warnings: [],
    next_statuses: [],
    ...over,
  });

export const decision = (localId: string, over: Over = {}) =>
  base(localId, {
    wording: `Entscheid ${localId}`,
    status: "in-force",
    decided_on: "2030-03-12",
    decided_by: [],
    can_end: false,
    ...over,
  });

export const risk = (localId: string, over: Over = {}) =>
  base(localId, {
    title: `Risiko ${localId}`,
    status: "open",
    likelihood: "medium",
    impact: "high",
    next_statuses: [],
    ...over,
  });

/** One record of each kind, each in another column, with owners from the fake directory. */
export function sampleWork() {
  return {
    "/actions": [
      action("ACT-001", {
        title: "Plakat gestalten",
        owner_user_id: BERND,
        due_date: "2030-05-18",
        workstream_id: STREAM_GROUND,
      }),
      action("ACT-002", { title: "Sponsorenliste nachführen", status: "in-progress" }),
      action("ACT-003", { title: "Festzelt bestellen", status: "blocked", owner_user_id: CLARA }),
      action("ACT-004", { title: "Flyer drucken", status: "done", due_date: "2030-01-10" }),
      action("ACT-005", { title: "Hallenplan zeichnen", status: "canceled" }),
      action("ACT-006", {
        title: "Rechnung bezahlen",
        due_date: "2020-02-01",
        owner_user_id: BERND,
      }),
    ],
    "/commitments": [
      commitment("COM-001", { text: "Bäckerei liefert 120 Zöpfe", due_date: "2030-06-01" }),
    ],
    "/milestones": [milestone("MIL-001", { title: "Programm steht", date: "2030-07-01" })],
    "/decisions": [decision("DEC-001", { wording: "Das Fest findet bei jedem Wetter statt." })],
    "/risks": [risk("RSK-001", { title: "Regen ohne Ausweichraum", status: "occurred" })],
  };
}
