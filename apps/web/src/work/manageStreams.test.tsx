import { act, renderHook } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it } from "vitest";
import { Router } from "../router/Router";
import { manageStreamsAddress, useManageStreams } from "./manageStreams";

const wrapper = ({ children }: { children: ReactNode }) => <Router>{children}</Router>;

function mount(url: string) {
  window.history.replaceState(null, "", url);
  return renderHook(() => useManageStreams(), { wrapper });
}

afterEach(() => window.history.replaceState(null, "", "/"));

describe("useManageStreams", () => {
  it("is closed by default and opens through the query key", () => {
    expect(mount("/events/e1/work").result.current.isOpen).toBe(false);
    expect(mount("/events/e1/work?manage=streams").result.current.isOpen).toBe(true);
  });

  it("ignores another value of the key", () => {
    expect(mount("/events/e1/work?manage=other").result.current.isOpen).toBe(false);
  });

  it("opens with a history entry and keeps the filters and the view", () => {
    const { result } = mount("/events/e1/work?view=table&stream=w1");
    const before = window.history.length;
    act(() => result.current.open());
    expect(result.current.isOpen).toBe(true);
    expect(new URLSearchParams(window.location.search).get("view")).toBe("table");
    expect(new URLSearchParams(window.location.search).get("stream")).toBe("w1");
    expect(window.history.length).toBe(before + 1);
  });

  it("closes without a history entry and keeps the rest of the query", () => {
    const { result } = mount("/events/e1/work?view=table&manage=streams");
    const before = window.history.length;
    act(() => result.current.close());
    expect(result.current.isOpen).toBe(false);
    expect(window.location.search).toBe("?view=table");
    expect(window.history.length).toBe(before);
  });

  it("names the address that the command palette goes to", () => {
    expect(manageStreamsAddress("e 1")).toBe("/events/e%201/work?manage=streams");
  });
});
