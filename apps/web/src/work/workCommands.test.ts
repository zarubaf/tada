import { describe, expect, it } from "vitest";
import { createApi } from "../api/client";
import { json, problem } from "../test/fakeWorkServer";
import { createWorkCommands } from "./workCommands";

interface Sent {
  method: string;
  path: string;
  body: Record<string, unknown>;
  answer: (response: Response) => void;
}

/** A server that records each request and answers when the test says so. */
function server() {
  const sent: Sent[] = [];
  const fetch = async (request: Request) => {
    const path = new URL(request.url).pathname;
    const body = JSON.parse(await request.clone().text());
    return new Promise<Response>((answer) => {
      sent.push({ method: request.method, path, body, answer });
    });
  };
  const api = createApi(fetch as unknown as typeof globalThis.fetch);
  return { sent, commands: createWorkCommands(api, "e1") };
}

const tick = () => new Promise((resolve) => setTimeout(resolve, 20));
const record = (id: string, version: number, over: object = {}) =>
  json(200, { id, version, ...over });

describe("work commands", () => {
  it("changes the status of an action and returns the new record", async () => {
    const { sent, commands } = server();
    const result = commands.action.changeStatus({ id: "a1", version: 2 }, "done");
    await tick();
    expect(sent[0]).toMatchObject({
      method: "PATCH",
      path: "/api/v1/events/e1/actions/a1",
      body: { status: "done", expected_version: 2 },
    });
    sent[0]?.answer(record("a1", 3, { status: "done" }));
    expect(await result).toEqual({ ok: true, record: { id: "a1", version: 3, status: "done" } });
  });

  it("sends the field of each command", async () => {
    const { sent, commands } = server();
    const t = { id: "x", version: 1 };
    const calls = [
      commands.action.changeOwner(t, "u2"),
      commands.commitment.changeDue(t, "2030-01-01"),
      commands.milestone.changeDue(t, "2030-02-02"),
      commands.decision.changeWorkstream(t, null),
      commands.risk.rename(t, "Neu"),
      commands.commitment.rename(t, "Text"),
    ];
    // They are one record, so they run one after the other.
    for (let i = 0; i < calls.length; i++) {
      await tick();
      sent[i]?.answer(record("x", i + 2));
    }
    await Promise.all(calls);
    expect(sent.map((s) => [s.path.split("/")[5], s.body])).toEqual([
      ["actions", { owner_user_id: "u2", expected_version: 1 }],
      ["commitments", { due_date: "2030-01-01", expected_version: 2 }],
      ["milestones", { date: "2030-02-02", expected_version: 3 }],
      ["decisions", { workstream_id: null, expected_version: 4 }],
      ["risks", { title: "Neu", expected_version: 5 }],
      ["commitments", { text: "Text", expected_version: 6 }],
    ]);
  });

  it("types a conflict, a refusal, a missing record and an invalid value", async () => {
    const { sent, commands } = server();
    const cases = [
      ["record-version-conflict", 409, "conflict"],
      ["forbidden", 403, "forbidden"],
      ["not-found", 404, "not-found"],
      ["validation-failed", 422, "validation"],
      ["invalid-transition", 409, "other"],
    ] as const;
    for (const [i, [code, status, reason]] of cases.entries()) {
      const result = commands.risk.changeStatus({ id: `r${i}`, version: 1 }, "closed");
      await tick();
      sent[i]?.answer(problem(status, code));
      const done = await result;
      expect(done).toMatchObject({ ok: false, failure: { reason } });
      if (!done.ok) {
        expect(done.failure.message).not.toBe("");
      }
    }
  });

  it("reports a lost connection as a failure", async () => {
    const api = createApi((async () => {
      throw new Error("offline");
    }) as unknown as typeof globalThis.fetch);
    const result = await createWorkCommands(api, "e1").action.rename({ id: "a1", version: 1 }, "x");
    expect(result).toMatchObject({ ok: false, failure: { reason: "other" } });
  });

  it("runs the commands of one record one after the other with the version of the answer", async () => {
    const { sent, commands } = server();
    const t = { id: "a1", version: 1 };
    const first = commands.action.changeStatus(t, "in-progress");
    const second = commands.action.changeOwner(t, "u2");
    await tick();
    expect(sent).toHaveLength(1);
    sent[0]?.answer(record("a1", 5));
    await first;
    await tick();
    expect(sent).toHaveLength(2);
    expect(sent[1]?.body).toEqual({ owner_user_id: "u2", expected_version: 5 });
    sent[1]?.answer(record("a1", 6));
    expect(await second).toMatchObject({ ok: true });
  });

  it("falls back to the version of the caller after a failure", async () => {
    const { sent, commands } = server();
    const first = commands.action.rename({ id: "a1", version: 1 }, "A");
    const second = commands.action.rename({ id: "a1", version: 1 }, "B");
    await tick();
    sent[0]?.answer(problem(409, "record-version-conflict"));
    await first;
    await tick();
    expect(sent[1]?.body).toMatchObject({ expected_version: 1 });
    sent[1]?.answer(record("a1", 2));
    await second;
  });

  it("keeps the newest version after a refusal that is not a version conflict", async () => {
    const { sent, commands } = server();
    const t = { id: "a1", version: 1 };
    const rename = commands.action.rename(t, "A");
    const due = commands.action.changeDue(t, "2030-01-01");
    const status = commands.action.changeStatus(t, "done");
    await tick();
    sent[0]?.answer(record("a1", 2));
    await rename;
    await tick();
    sent[1]?.answer(problem(422, "validation-failed"));
    await due;
    await tick();
    expect(sent[2]?.body).toMatchObject({ expected_version: 2 });
    sent[2]?.answer(record("a1", 3));
    await status;
  });

  it("forgets the newest version after a conflict or a missing record", async () => {
    const { sent, commands } = server();
    const first = commands.action.rename({ id: "a1", version: 1 }, "A");
    const second = commands.action.rename({ id: "a1", version: 1 }, "B");
    const third = commands.action.rename({ id: "a1", version: 1 }, "C");
    await tick();
    sent[0]?.answer(record("a1", 2));
    await first;
    await tick();
    sent[1]?.answer(problem(409, "record-version-conflict"));
    await second;
    await tick();
    expect(sent[2]?.body).toMatchObject({ expected_version: 1 });
    sent[2]?.answer(record("a1", 3));
    await third;
  });

  it("runs the commands of different records at the same time", async () => {
    const { sent, commands } = server();
    const first = commands.action.rename({ id: "a1", version: 1 }, "A");
    const second = commands.risk.rename({ id: "r1", version: 1 }, "B");
    const third = commands.action.rename({ id: "a2", version: 1 }, "C");
    await tick();
    expect(sent).toHaveLength(3);
    sent.forEach((s, i) => {
      s.answer(record(`id${i}`, 2));
    });
    await Promise.all([first, second, third]);
  });
});

describe("the commands of an action beyond the first set", () => {
  it("changes the start date, the description and the milestone, and can clear each", async () => {
    const { sent, commands } = server();
    const t = { id: "a1", version: 1 };
    const calls = [
      commands.action.changeStart(t, "2030-05-01"),
      commands.action.changeStart(t, null),
      commands.action.changeDescription(t, "Text"),
      commands.action.changeDescription(t, null),
      commands.action.changeMilestone(t, "m1"),
      commands.action.changeMilestone(t, null),
    ];
    for (let i = 0; i < calls.length; i++) {
      await tick();
      sent[i]?.answer(record("a1", i + 2));
    }
    await Promise.all(calls);
    expect(sent.map((s) => s.body)).toEqual([
      { start_date: "2030-05-01", expected_version: 1 },
      { start_date: null, expected_version: 2 },
      { description: "Text", expected_version: 3 },
      { description: null, expected_version: 4 },
      { milestone_id: "m1", expected_version: 5 },
      { milestone_id: null, expected_version: 6 },
    ]);
  });

  it("creates an action and returns it", async () => {
    const { sent, commands } = server();
    const result = commands.action.create({ title: "Neu", owner_user_id: "u1" });
    await tick();
    expect(sent[0]).toMatchObject({
      method: "POST",
      path: "/api/v1/events/e1/actions",
      body: { title: "Neu", owner_user_id: "u1" },
    });
    sent[0]?.answer(json(201, { id: "a9", version: 1 }));
    expect(await result).toEqual({ ok: true, record: { id: "a9", version: 1 } });
  });

  it("keeps the problem of a refused create, so that a form can show the invalid fields", async () => {
    const { sent, commands } = server();
    const result = commands.action.create({ title: "Neu", owner_user_id: "u1" });
    await tick();
    sent[0]?.answer(problem(422, "validation-failed", [{ pointer: "/due_date", code: "invalid" }]));
    expect(await result).toMatchObject({
      ok: false,
      failure: {
        reason: "validation",
        problem: { errors: [{ pointer: "/due_date", code: "invalid" }] },
      },
    });
  });
});

describe("the commands of a risk beyond the first set", () => {
  it("changes the rating, the mitigation and the description, and can clear a text", async () => {
    const { sent, commands } = server();
    const t = { id: "r1", version: 1 };
    const calls = [
      commands.risk.changeImpact(t, "high"),
      commands.risk.changeLikelihood(t, "low"),
      commands.risk.changeMitigation(t, "Zelt bestellen"),
      commands.risk.changeMitigation(t, null),
      commands.risk.changeDescription(t, "Text"),
      commands.risk.changeDescription(t, null),
    ];
    for (let i = 0; i < calls.length; i++) {
      await tick();
      sent[i]?.answer(record("r1", i + 2));
    }
    await Promise.all(calls);
    expect(sent.map((s) => [s.path, s.body])).toEqual([
      ["/api/v1/events/e1/risks/r1", { impact: "high", expected_version: 1 }],
      ["/api/v1/events/e1/risks/r1", { likelihood: "low", expected_version: 2 }],
      ["/api/v1/events/e1/risks/r1", { mitigation: "Zelt bestellen", expected_version: 3 }],
      ["/api/v1/events/e1/risks/r1", { mitigation: null, expected_version: 4 }],
      ["/api/v1/events/e1/risks/r1", { description: "Text", expected_version: 5 }],
      ["/api/v1/events/e1/risks/r1", { description: null, expected_version: 6 }],
    ]);
  });

  it("creates a risk and returns it", async () => {
    const { sent, commands } = server();
    const body = {
      id: "r9",
      title: "Regen",
      impact: "high",
      likelihood: "medium",
      owner_user_id: "u1",
    } as const;
    const result = commands.risk.create(body);
    await tick();
    expect(sent[0]).toMatchObject({ method: "POST", path: "/api/v1/events/e1/risks", body });
    sent[0]?.answer(json(201, { id: "r9", version: 1 }));
    expect(await result).toEqual({ ok: true, record: { id: "r9", version: 1 } });
  });

  it("keeps the problem of a refused create, so that a form can show the invalid fields", async () => {
    const { sent, commands } = server();
    const result = commands.risk.create({
      title: "Regen",
      impact: "high",
      likelihood: "low",
      owner_user_id: "u1",
    });
    await tick();
    sent[0]?.answer(
      problem(422, "validation-failed", [{ pointer: "/owner", code: "unknown-member" }]),
    );
    expect(await result).toMatchObject({
      ok: false,
      failure: { reason: "validation", problem: { errors: [{ pointer: "/owner" }] } },
    });
  });
});

describe("the commands of a milestone and a decision beyond the first set", () => {
  it("changes the description of a milestone and can clear it", async () => {
    const { sent, commands } = server();
    const t = { id: "m1", version: 1 };
    const calls = [
      commands.milestone.changeDescription(t, "Text"),
      commands.milestone.changeDescription(t, null),
    ];
    for (let i = 0; i < calls.length; i++) {
      await tick();
      sent[i]?.answer(record("m1", i + 2));
    }
    await Promise.all(calls);
    expect(sent.map((s) => s.body)).toEqual([
      { description: "Text", expected_version: 1 },
      { description: null, expected_version: 2 },
    ]);
  });

  it("creates a milestone and a decision with the ID that the caller chose", async () => {
    const { sent, commands } = server();
    const milestone = commands.milestone.create({
      id: "m9",
      title: "Programm steht",
      date: "2030-07-01",
      owner_user_id: "u1",
    });
    const decision = commands.decision.create({
      id: "d9",
      wording: "Das Fest findet statt.",
      decided_on: "2030-03-12",
      decided_by: [{ kind: "person", id: "p1" }],
      owner_user_id: "u1",
    });
    await tick();
    expect(sent.map((s) => [s.method, s.path])).toEqual([
      ["POST", "/api/v1/events/e1/milestones"],
      ["POST", "/api/v1/events/e1/decisions"],
    ]);
    sent[0]?.answer(json(201, { id: "m9", version: 1 }));
    sent[1]?.answer(json(201, { id: "d9", version: 1 }));
    expect(await milestone).toEqual({ ok: true, record: { id: "m9", version: 1 } });
    expect(await decision).toEqual({ ok: true, record: { id: "d9", version: 1 } });
  });

  it("supersedes a decision with the successor and the version, and returns the successor", async () => {
    const { sent, commands } = server();
    const result = commands.decision.supersede(
      { id: "d1", version: 3 },
      { wording: "Neu", decided_on: "2030-04-01", decided_by: [], owner_user_id: "u1" },
    );
    await tick();
    expect(sent[0]).toMatchObject({
      method: "POST",
      path: "/api/v1/events/e1/decisions/d1/supersede",
      body: {
        successor: {
          wording: "Neu",
          decided_on: "2030-04-01",
          decided_by: [],
          owner_user_id: "u1",
        },
        expected_version: 3,
      },
    });
    sent[0]?.answer(json(201, { id: "d2", version: 1 }));
    expect(await result).toEqual({ ok: true, record: { id: "d2", version: 1 } });
  });

  it("does not take the version of the successor for the one of the replaced decision", async () => {
    const { sent, commands } = server();
    const replace = commands.decision.supersede(
      { id: "d1", version: 3 },
      { wording: "Neu", decided_on: "2030-04-01", decided_by: [], owner_user_id: "u1" },
    );
    // Queued behind the replacement, with the version that its caller knows.
    const owner = commands.decision.changeOwner({ id: "d1", version: 5 }, "u2");
    await tick();
    sent[0]?.answer(json(201, { id: "d2", version: 7 }));
    await replace;
    await tick();
    expect(sent[1]).toMatchObject({ body: { owner_user_id: "u2", expected_version: 5 } });
    sent[1]?.answer(json(409, { code: "record-version-conflict" }));
    await owner;
  });

  it("revokes a decision with the reason and the version, and keeps a refused reason", async () => {
    const { sent, commands } = server();
    const result = commands.decision.revoke({ id: "d1", version: 3 }, "Nicht mehr aktuell");
    await tick();
    expect(sent[0]).toMatchObject({
      method: "POST",
      path: "/api/v1/events/e1/decisions/d1/revoke",
      body: { reason: "Nicht mehr aktuell", expected_version: 3 },
    });
    sent[0]?.answer(problem(422, "validation-failed", [{ pointer: "/reason", code: "required" }]));
    expect(await result).toMatchObject({
      ok: false,
      failure: { reason: "validation", problem: { errors: [{ pointer: "/reason" }] } },
    });
  });
});

describe("the commands of a commitment beyond the first set", () => {
  it("creates a commitment and keeps the problem of a refused create", async () => {
    const { sent, commands } = server();
    const body = {
      text: "Zöpfe liefern",
      owner_user_id: "u1",
      promisor: { kind: "person", id: "p1" },
    } as const;
    const created = commands.commitment.create(body);
    await tick();
    expect(sent[0]).toMatchObject({
      method: "POST",
      path: "/api/v1/events/e1/commitments",
      body,
    });
    sent[0]?.answer(json(201, { id: "c9", version: 1 }));
    expect(await created).toEqual({ ok: true, record: { id: "c9", version: 1 } });

    const refused = commands.commitment.create(body);
    await tick();
    sent[1]?.answer(problem(422, "validation-failed", [{ pointer: "/promisor", code: "unknown" }]));
    expect(await refused).toMatchObject({
      ok: false,
      failure: { reason: "validation", problem: { errors: [{ pointer: "/promisor" }] } },
    });
  });

  it("makes a commitment firm with a reason, in the queue of the record", async () => {
    const { sent, commands } = server();
    const t = { id: "c1", version: 1 };
    const rename = commands.commitment.rename(t, "Neu");
    const firm = commands.commitment.makeFirm(t, "Auftrag unterschrieben");
    await tick();
    sent[0]?.answer(record("c1", 2));
    await tick();
    // It waits for the rename and takes the version of its answer.
    expect(sent[1]).toMatchObject({
      method: "POST",
      path: "/api/v1/events/e1/commitments/c1/firm",
      body: { reason: "Auftrag unterschrieben", expected_version: 2 },
    });
    sent[1]?.answer(record("c1", 3, { status: "firm" }));
    expect(await rename).toMatchObject({ ok: true });
    expect(await firm).toEqual({ ok: true, record: { id: "c1", version: 3, status: "firm" } });
  });

  it("types the failures of make firm like the others", async () => {
    const { sent, commands } = server();
    const firm = commands.commitment.makeFirm({ id: "c1", version: 1 }, "x");
    await tick();
    sent[0]?.answer(problem(409, "record-version-conflict"));
    expect(await firm).toMatchObject({ ok: false, failure: { reason: "conflict" } });
  });
});

describe("what a kind does not offer", () => {
  it("has no command for a field that the API does not take", () => {
    const { commands } = server();
    // @ts-expect-error a decision changes its status through supersede and revoke
    void commands.decision.changeStatus;
    // @ts-expect-error the day of a decision never changes
    void commands.decision.changeDue;
    // @ts-expect-error a decision has no title to change
    void commands.decision.rename;
    // @ts-expect-error a risk has no date
    void commands.risk.changeDue;
    // @ts-expect-error only "make firm" sets the status firm
    void commands.commitment.changeStatus({ id: "c", version: 1 }, "firm");
    expect(Object.keys(commands.decision).sort()).toEqual([
      "changeOwner",
      "changeWorkstream",
      "create",
      "revoke",
      "supersede",
    ]);
  });
});
