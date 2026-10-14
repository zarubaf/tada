import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { Api, Event } from "../api/client";
import { useEventList } from "./useEvents";

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
