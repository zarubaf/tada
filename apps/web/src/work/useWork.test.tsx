import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { describe, expect, it } from "vitest";
import { createApi } from "../api/client";
import { Router } from "../router/Router";
import { SessionProvider } from "../session/SessionProvider";
import { json, ME, problem } from "../test/fakeWorkServer";
import { useWork } from "./useWork";

const own = { organization_id: "o1", name: "Fliegergruppe Testwil", role: "member" };
const session = {
  user_id: ME,
  display_name: "Anna Muster",
  organization: own,
  memberships: [own],
};

type Answer = (url: URL) => Response | Promise<Response>;

const rec = (id: string, over: object = {}) => ({
  id,
  local_id: id.toUpperCase(),
  event_id: "ea",
  owner_user_id: ME,
  redacted: false,
  version: 1,
  status: "open",
  title: id,
  text: id,
  wording: id,
  date: "2030-01-01",
  decided_on: "2030-01-01",
  can_change: true,
  ...over,
});

/** A fake server: the lists answer by `event/kind`, and the test can replace an answer. */
function server(answers: Record<string, Answer> = {}) {
  const calls: string[] = [];
  /** The requests of the directory, apart from the five lists. */
  const directory: string[] = [];
  const table: Record<string, Answer> = { ...answers };
  const fetch = async (request: Request) => {
    const url = new URL(request.url);
    const path = url.pathname;
    if (path.endsWith("/session")) {
      return json(200, session);
    }
    if (path.endsWith("/api/v1/members")) {
      directory.push("members");
      return json(200, {
        items: [{ user_id: ME, display_name: "Anna Muster", role: "member", version: 1 }],
      });
    }
    if (path.endsWith("/memberships") || path.endsWith("/workstreams")) {
      directory.push(path.split("/").pop() ?? "");
      return json(200, { items: [] });
    }
    const patch = /events\/([^/]+)\/([a-z]+)\/([^/]+)$/.exec(path);
    if (request.method === "PATCH" && patch) {
      const answer = table[`PATCH ${patch[1]}/${patch[2]}/${patch[3]}`];
      if (!answer) {
        throw new Error(`unexpected ${request.method} ${path}`);
      }
      return answer(url);
    }
    const match = /events\/([^/]+)\/([a-z]+)$/.exec(path);
    if (match) {
      calls.push(`${match[1]}/${match[2]}${url.search}`);
      const answer = table[`${match[1]}/${match[2]}`];
      return answer ? answer(url) : json(200, { items: [] });
    }
    throw new Error(`unexpected ${request.method} ${path}`);
  };
  const api = createApi(fetch as unknown as typeof globalThis.fetch);
  return { api, calls, directory, table };
}

function mount(api: ReturnType<typeof createApi>, initial = "ea") {
  const wrapper = ({ children }: { children: ReactNode }) => (
    <Router>
      <SessionProvider api={api}>{children}</SessionProvider>
    </Router>
  );
  return renderHook(({ eventId }) => useWork(api, eventId), {
    wrapper,
    initialProps: { eventId: initial },
  });
}

const states = (work: ReturnType<typeof useWork>) =>
  Object.fromEntries(Object.entries(work.kinds).map(([kind, s]) => [kind, s.kind]));

describe("useWork", () => {
  it("starts the five kinds together, before any answer", async () => {
    const gate: { open: () => void } = { open: () => undefined };
    const opened = new Promise<void>((resolve) => {
      gate.open = resolve;
    });
    const answer: Answer = async () => {
      await opened;
      return json(200, { items: [] });
    };
    const { api, calls } = server(
      Object.fromEntries(
        ["actions", "commitments", "milestones", "decisions", "risks"].map((k) => [
          `ea/${k}`,
          answer,
        ]),
      ),
    );
    const { result } = mount(api);
    await waitFor(() => expect(calls).toHaveLength(5));
    expect(states(result.current)).toEqual({
      action: "loading",
      commitment: "loading",
      milestone: "loading",
      decision: "loading",
      risk: "loading",
    });
    gate.open();
    await waitFor(() => expect(result.current.kinds.risk.kind).toBe("loaded"));
  });

  it("loads every page of every kind and maps the records with the names", async () => {
    const { api, calls } = server({
      "ea/actions": (url) =>
        url.searchParams.get("cursor") === "p2"
          ? json(200, { items: [rec("act-2")] })
          : json(200, { items: [rec("act-1")], next_cursor: "p2" }),
      "ea/risks": () => json(200, { items: [rec("rsk-1"), rec("rsk-1")] }),
    });
    const { result } = mount(api);
    await waitFor(() => expect(result.current.directory.kind).toBe("loaded"));
    await waitFor(() => expect(result.current.items).toHaveLength(3));
    expect(result.current.items.map((i) => i.readableId).sort()).toEqual([
      "ACT-1",
      "ACT-2",
      "RSK-1",
    ]);
    expect(result.current.items[0]?.owner).toEqual({ id: ME, name: "Anna Muster" });
    expect(calls.filter((c) => c.startsWith("ea/actions"))).toEqual([
      "ea/actions?limit=200",
      "ea/actions?limit=200&cursor=p2",
    ]);
  });

  it("ends a list whose server repeats the cursor", async () => {
    const { api, calls } = server({
      "ea/commitments": () =>
        json(200, { items: [rec("com-1", { status: "firm" })], next_cursor: "same" }),
    });
    const { result } = mount(api);
    await waitFor(() => expect(result.current.kinds.commitment.kind).toBe("loaded"));
    expect(calls.filter((c) => c.startsWith("ea/commitments"))).toHaveLength(2);
  });

  it("shows a failed kind next to the others and retries only that kind", async () => {
    const { api, calls, table } = server({
      "ea/milestones": () => problem(500, "internal"),
      "ea/actions": () => json(200, { items: [rec("act-1")] }),
    });
    const { result } = mount(api);
    await waitFor(() => expect(result.current.kinds.milestone.kind).toBe("failed"));
    expect(result.current.kinds.milestone).toMatchObject({ message: expect.any(String) });
    expect(states(result.current).action).toBe("loaded");
    expect(result.current.items.map((i) => i.readableId)).toEqual(["ACT-1"]);

    table["ea/milestones"] = () => json(200, { items: [rec("mil-1")] });
    const before = calls.length;
    let ok = false;
    await act(async () => {
      ok = await result.current.reloadKind("milestone");
    });
    expect(ok).toBe(true);
    expect(calls.slice(before)).toEqual(["ea/milestones?limit=200"]);
    expect(result.current.items.map((i) => i.readableId).sort()).toEqual(["ACT-1", "MIL-1"]);
  });

  it("drops the answer of an older load", async () => {
    const waiting: ((response: Response) => void)[] = [];
    const { api } = server({
      "ea/actions": () => new Promise<Response>((resolve) => waiting.push(resolve)),
    });
    const { result } = mount(api);
    await waitFor(() => expect(waiting).toHaveLength(1));
    act(() => {
      void result.current.reloadKind("action");
    });
    await waitFor(() => expect(waiting).toHaveLength(2));
    waiting[1]?.(json(200, { items: [rec("act-new")] }));
    await waitFor(() => expect(result.current.items.map((i) => i.readableId)).toEqual(["ACT-NEW"]));
    waiting[0]?.(json(200, { items: [rec("act-old")] }));
    await new Promise((resolve) => setTimeout(resolve, 30));
    expect(result.current.items.map((i) => i.readableId)).toEqual(["ACT-NEW"]);
  });

  it("drops the answer of the event that the page left", async () => {
    const waiting: ((response: Response) => void)[] = [];
    const { api } = server({
      "ea/actions": () => new Promise<Response>((resolve) => waiting.push(resolve)),
      "eb/actions": () => json(200, { items: [rec("act-b")] }),
    });
    const { result, rerender } = mount(api);
    await waitFor(() => expect(waiting).toHaveLength(1));
    rerender({ eventId: "eb" });
    await waitFor(() => expect(result.current.items.map((i) => i.readableId)).toEqual(["ACT-B"]));
    waiting[0]?.(json(200, { items: [rec("act-a")] }));
    await new Promise((resolve) => setTimeout(resolve, 30));
    expect(result.current.items.map((i) => i.readableId)).toEqual(["ACT-B"]);
  });

  it("does not show the records of the previous event while the next one loads", async () => {
    const { api } = server({
      "ea/actions": () => json(200, { items: [rec("act-a")] }),
      "eb/actions": () => new Promise<Response>(() => undefined),
    });
    const { result, rerender } = mount(api);
    await waitFor(() => expect(result.current.items).toHaveLength(1));
    rerender({ eventId: "eb" });
    expect(result.current.kinds.action.kind).toBe("loading");
    expect(result.current.items).toHaveLength(0);
  });

  it("refreshes a kind without the loading state, and replaces a record", async () => {
    const { api, calls, table } = server({
      "ea/actions": () => json(200, { items: [rec("act-1")] }),
    });
    const { result } = mount(api);
    await waitFor(() => expect(result.current.items).toHaveLength(1));

    act(() => {
      result.current.replace("action", rec("act-1", { title: "Neu", version: 2 }) as never);
    });
    expect(result.current.items[0]).toMatchObject({ title: "Neu", version: 2 });

    let release: () => void = () => undefined;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    table["ea/actions"] = async () => {
      await gate;
      return json(200, { items: [rec("act-1", { title: "Server", version: 3 })] });
    };
    const before = calls.length;
    let done: Promise<boolean> = Promise.resolve(false);
    act(() => {
      done = result.current.refresh("action");
    });
    await waitFor(() => expect(calls.length).toBeGreaterThan(before));
    // The request is pending: the list stays on screen.
    expect(result.current.kinds.action.kind).toBe("loaded");
    expect(result.current.items[0]?.title).toBe("Neu");
    release();
    await act(async () => {
      await done;
    });
    expect(result.current.items[0]?.title).toBe("Server");
  });

  it("keeps a record that a command returned when an older list arrives after it", async () => {
    const { api, calls, table } = server({
      "ea/actions": () => json(200, { items: [rec("act-1", { version: 4 })] }),
      "PATCH ea/actions/act-1": () => json(200, rec("act-1", { title: "Neu", version: 5 })),
    });
    const { result } = mount(api);
    await waitFor(() => expect(result.current.items).toHaveLength(1));
    let release: () => void = () => undefined;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    table["ea/actions"] = async () => {
      await gate;
      return json(200, { items: [rec("act-1", { version: 4 })] });
    };
    const before = calls.length;
    let done: Promise<boolean> = Promise.resolve(false);
    act(() => {
      done = result.current.refresh("action");
    });
    await waitFor(() => expect(calls.length).toBeGreaterThan(before));
    await act(async () => {
      await result.current.commands.action.rename({ id: "act-1", version: 4 }, "Neu");
    });
    expect(result.current.items[0]).toMatchObject({ title: "Neu", version: 5 });
    release();
    await act(async () => {
      await done;
    });
    expect(result.current.items[0]).toMatchObject({ title: "Neu", version: 5 });
  });

  it("puts the record of a successful command into the list, and leaves it after a failure", async () => {
    const { api, table } = server({
      "ea/actions": () => json(200, { items: [rec("act-1")] }),
      "PATCH ea/actions/act-1": () => json(200, rec("act-1", { title: "Neu", version: 2 })),
    });
    const { result } = mount(api);
    await waitFor(() => expect(result.current.items).toHaveLength(1));
    let outcome: { ok: boolean } | undefined;
    await act(async () => {
      outcome = await result.current.commands.action.rename({ id: "act-1", version: 1 }, "Neu");
    });
    expect(outcome?.ok).toBe(true);
    expect(result.current.items[0]).toMatchObject({ title: "Neu", version: 2 });

    table["PATCH ea/actions/act-1"] = () => problem(422, "validation-failed");
    await act(async () => {
      outcome = await result.current.commands.action.rename({ id: "act-1", version: 2 }, "");
    });
    expect(outcome?.ok).toBe(false);
    expect(result.current.items[0]).toMatchObject({ title: "Neu", version: 2 });
  });

  it("keeps the list when a refresh fails", async () => {
    const { api, table } = server({
      "ea/actions": () => json(200, { items: [rec("act-1")] }),
    });
    const { result } = mount(api);
    await waitFor(() => expect(result.current.items).toHaveLength(1));
    table["ea/actions"] = () => problem(500, "internal");
    let ok = true;
    await act(async () => {
      ok = await result.current.refresh("action");
    });
    expect(ok).toBe(false);
    expect(result.current.kinds.action.kind).toBe("loaded");
  });

  it("reloads the five kinds and the directory", async () => {
    const { api, calls, directory } = server();
    const { result } = mount(api);
    await waitFor(() => expect(result.current.directory.kind).toBe("loaded"));
    await waitFor(() => expect(states(result.current).risk).toBe("loaded"));
    // The first load asks for the directory once, not once per kind.
    expect(directory.sort()).toEqual(["members", "memberships", "workstreams"]);
    const before = calls.length;
    await act(async () => {
      await result.current.reload();
    });
    expect(calls.length - before).toBe(5);
    expect(directory.sort()).toEqual([
      "members",
      "members",
      "memberships",
      "memberships",
      "workstreams",
      "workstreams",
    ]);
  });
});
