import { act, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Api, Event } from "../api/client";
import { eventKey, writeRemembered } from "./remembered";
import { type EventList, mostRecentOpen, useEventList, useScopedEvent } from "./useEvents";

interface Pending {
  resolve: (events: Event[]) => void;
}

/** An API whose list request stays open until the test answers it. */
function slowApi(): { api: Api; requests: Pending[] } {
  const requests: Pending[] = [];
  const GET = vi.fn(
    () =>
      new Promise((resolve) => {
        requests.push({
          resolve: (events) => resolve({ data: { items: events }, error: undefined }),
        });
      }),
  );
  return { api: { GET } as unknown as Api, requests };
}

const event = (id: string): Event => ({ id, name: id }) as unknown as Event;

describe("useEventList", () => {
  it("drops the answer of a request of the previous organization", async () => {
    const { api, requests } = slowApi();
    const { result, rerender } = renderHook(({ organization }) => useEventList(api, organization), {
      initialProps: { organization: "org-a" as string | undefined },
    });

    act(() => result.current.load());
    rerender({ organization: "org-b" });
    await act(async () => requests[0]?.resolve([event("old")]));

    expect(result.current.list).toEqual({ kind: "idle" });
  });

  it("asks again for the new organization while the old request still runs", async () => {
    const { api, requests } = slowApi();
    const { result, rerender } = renderHook(({ organization }) => useEventList(api, organization), {
      initialProps: { organization: "org-a" as string | undefined },
    });

    act(() => result.current.load());
    rerender({ organization: "org-b" });
    act(() => result.current.load());
    expect(requests).toHaveLength(2);
    await act(async () => requests[1]?.resolve([event("new")]));
    await act(async () => requests[0]?.resolve([event("old")]));

    expect(result.current.list).toEqual({ kind: "loaded", events: [event("new")] });
  });
});

describe("useEventList, more than one page", () => {
  it("loads every page with the largest page size", async () => {
    const queries: unknown[] = [];
    const pages = [
      { items: [event("a")], next_cursor: "c1" },
      { items: [event("b")], next_cursor: null },
    ];
    const GET = vi.fn(async (_path: string, init: { params: { query: unknown } }) => {
      queries.push(init.params.query);
      return { data: pages[queries.length - 1], error: undefined };
    });
    const { result } = renderHook(() => useEventList({ GET } as unknown as Api, "org-a"));

    await act(async () => result.current.load());

    await vi.waitFor(() =>
      expect(result.current.list).toEqual({ kind: "loaded", events: [event("a"), event("b")] }),
    );
    expect(queries).toEqual([{ limit: 200 }, { limit: 200, cursor: "c1" }]);
  });
});

const full = (id: string, createdAt: string, status = "active"): Event =>
  ({ id, name: id, created_at: createdAt, status }) as unknown as Event;

/** An API that knows the given events by ID and answers 404 for the others. */
function knownApi(known: Event[]): Api {
  const GET = vi.fn(async (_path: string, init: { params: { path: { event_id: string } } }) => {
    const data = known.find((event) => event.id === init.params.path.event_id);
    return data
      ? { data, response: { status: 200 } }
      : { data: undefined, response: { status: 404 } };
  });
  return { GET } as unknown as Api;
}

describe("mostRecentOpen", () => {
  it("breaks a tie of the creation time by the ID", () => {
    const a = full("a", "2026-01-01T00:00:00Z");
    const b = full("b", "2026-01-01T00:00:00Z");
    expect(mostRecentOpen([a, b])?.id).toBe("b");
    expect(mostRecentOpen([b, a])?.id).toBe("b");
  });

  it("takes the newest active event and skips the closed ones", () => {
    const events = [
      full("old", "2026-01-01T00:00:00Z"),
      full("new", "2026-03-01T00:00:00Z"),
      full("closed", "2026-05-01T00:00:00Z", "closed"),
    ];
    expect(mostRecentOpen(events)?.id).toBe("new");
    expect(mostRecentOpen([full("closed", "2026-05-01T00:00:00Z", "closed")])).toBeUndefined();
  });
});

describe("useScopedEvent", () => {
  const org = "org-a";
  const events = [full("a", "2026-01-01T00:00:00Z"), full("b", "2026-02-01T00:00:00Z")];
  const loaded: EventList = { kind: "loaded", events };

  afterEach(() => window.localStorage.clear());

  it("prefers the event of the route", async () => {
    writeRemembered(eventKey(org), "b");
    const { result } = renderHook(() =>
      useScopedEvent(knownApi(events), "a", org, { list: loaded, load: vi.fn() }),
    );
    await vi.waitFor(() => expect(result.current.event?.id).toBe("a"));
  });

  it("falls back to the remembered event", async () => {
    writeRemembered(eventKey(org), "a");
    const { result } = renderHook(() =>
      useScopedEvent(knownApi(events), undefined, org, { list: loaded, load: vi.fn() }),
    );
    await vi.waitFor(() => expect(result.current.event?.id).toBe("a"));
  });

  it("falls back to the most recent open event and asks for the list", async () => {
    const load = vi.fn();
    const { result, rerender } = renderHook(
      ({ list }) => useScopedEvent(knownApi(events), undefined, org, { list, load }),
      { initialProps: { list: { kind: "idle" } as EventList } },
    );
    expect(result.current.event).toBeUndefined();
    expect(result.current.pending).toBe(true);
    expect(load).toHaveBeenCalled();
    rerender({ list: loaded });
    expect(result.current.event?.id).toBe("b");
  });

  it("falls back when the remembered event is gone", async () => {
    writeRemembered(eventKey(org), "gone");
    const { result } = renderHook(() =>
      useScopedEvent(knownApi(events), undefined, org, { list: loaded, load: vi.fn() }),
    );
    await vi.waitFor(() => expect(result.current.event?.id).toBe("b"));
  });

  it("gives a route event that the server does not find no other event", async () => {
    const api = knownApi(events);
    const load = vi.fn();
    const { result } = renderHook(() => useScopedEvent(api, "dead", org, { list: loaded, load }));
    await vi.waitFor(() => expect(api.GET).toHaveBeenCalled());
    await act(async () => undefined);
    expect(result.current.event).toBeUndefined();
    expect(result.current.pending).toBe(false);
    expect(load).not.toHaveBeenCalled();
  });

  it("is none without any open event", () => {
    const list: EventList = {
      kind: "loaded",
      events: [full("c", "2026-01-01T00:00:00Z", "closed")],
    };
    const { result } = renderHook(() =>
      useScopedEvent(knownApi([]), undefined, org, { list, load: vi.fn() }),
    );
    expect(result.current.event).toBeUndefined();
    expect(result.current.pending).toBe(false);
  });
});
