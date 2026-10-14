import { act, renderHook } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it } from "vitest";
import { Router } from "../router/Router";
import { parseBoardView, useBoardView } from "./boardView";

const wrapper = ({ children }: { children: ReactNode }) => <Router>{children}</Router>;

afterEach(() => window.history.replaceState(null, "", "/"));

describe("parseBoardView", () => {
  it("reads the status of the phone list and the open Abgebrochen", () => {
    expect(parseBoardView(new URLSearchParams("column=done&canceled=1"))).toEqual({
      column: "done",
      canceled: true,
    });
  });

  it("has defaults, and ignores what it does not know", () => {
    expect(parseBoardView(new URLSearchParams())).toEqual({ column: undefined, canceled: false });
    expect(parseBoardView(new URLSearchParams("column=nope&canceled=yes"))).toEqual({
      column: undefined,
      canceled: false,
    });
  });
});

describe("useBoardView", () => {
  it("writes the view into the query string with replace and keeps the other keys", () => {
    window.history.replaceState(null, "", "/events/e1/work?type=risk#x");
    const { result } = renderHook(() => useBoardView(), { wrapper });
    const before = window.history.length;
    act(() => result.current[1]({ column: "blocked" }));
    expect(window.location.search).toBe("?type=risk&column=blocked");
    expect(window.location.hash).toBe("#x");
    act(() => result.current[1]({ canceled: true }));
    expect(new URLSearchParams(window.location.search).get("canceled")).toBe("1");
    expect(result.current[0]).toEqual({ column: "blocked", canceled: true });
    expect(window.history.length).toBe(before);
  });

  it("leaves a key out at its default", () => {
    window.history.replaceState(null, "", "/?column=done&canceled=1");
    const { result } = renderHook(() => useBoardView(), { wrapper });
    act(() => result.current[1]({ column: undefined, canceled: false }));
    expect(window.location.search).toBe("");
  });
});
