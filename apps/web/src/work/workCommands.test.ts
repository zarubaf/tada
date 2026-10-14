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
    expect(Object.keys(commands.decision).sort()).toEqual(["changeOwner", "changeWorkstream"]);
  });
});
