import { act, renderHook } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it } from "vitest";
import { Router } from "../../router/Router";
import { useRecordRoute } from "./recordRoute";

const wrapper = ({ children }: { children: ReactNode }) => <Router>{children}</Router>;

function mount(url: string) {
  window.history.replaceState(null, "", url);
  return renderHook(() => useRecordRoute(), { wrapper });
}

afterEach(() => window.history.replaceState(null, "", "/"));

describe("useRecordRoute", () => {
  it("is closed without a hash and without the key to create", () => {
    expect(mount("/events/e1/work?view=table").result.current.route).toEqual({ mode: "closed" });
  });

  it("names the open record by the hash", () => {
    expect(mount("/events/e1/work#ACT-012").result.current.route).toEqual({
      mode: "open",
      id: "ACT-012",
    });
  });

  it("reads the ID of the hash in upper case, as the IDs are written", () => {
    expect(mount("/events/e1/work#act-012").result.current.route).toEqual({
      mode: "open",
      id: "ACT-012",
    });
  });

  it("creates the kind that the query names, before it opens a record", () => {
    expect(mount("/events/e1/work?new=action#ACT-012").result.current.route).toEqual({
      mode: "create",
      kind: "action",
    });
  });

  it("opens the create state of a risk", () => {
    expect(mount("/events/e1/work?new=risk").result.current.route).toEqual({
      mode: "create",
      kind: "risk",
    });
  });

  it("ignores a kind that it cannot create", () => {
    expect(mount("/events/e1/work?new=widget").result.current.route).toEqual({ mode: "closed" });
  });

  it("moves to another record with replace and keeps the query", () => {
    const { result } = mount("/events/e1/work?view=table#ACT-001");
    const before = window.history.length;
    act(() => result.current.open("ACT-002"));
    expect(window.location.pathname + window.location.search + window.location.hash).toBe(
      "/events/e1/work?view=table#ACT-002",
    );
    expect(window.history.length).toBe(before);
    expect(result.current.route).toEqual({ mode: "open", id: "ACT-002" });
  });

  it("closes by removing the hash and keeps the query", () => {
    const { result } = mount("/events/e1/work?view=table&type=action#ACT-001");
    const before = window.history.length;
    act(() => result.current.close());
    expect(window.location.search).toBe("?view=table&type=action");
    expect(window.location.hash).toBe("");
    expect(window.history.length).toBe(before);
  });

  it("starts to create with a new history entry, so that back closes the form", () => {
    const { result } = mount("/events/e1/work?view=table#ACT-001");
    const before = window.history.length;
    act(() => result.current.startCreate("action"));
    expect(window.location.search).toBe("?view=table&new=action");
    expect(window.location.hash).toBe("");
    expect(window.history.length).toBe(before + 1);
  });

  it("switches to the created record and drops the key to create", () => {
    const { result } = mount("/events/e1/work?view=table&new=action");
    act(() => result.current.open("ACT-020"));
    expect(window.location.search).toBe("?view=table");
    expect(window.location.hash).toBe("#ACT-020");
    expect(result.current.route).toEqual({ mode: "open", id: "ACT-020" });
  });

  it("closes the create state by dropping the key", () => {
    const { result } = mount("/events/e1/work?new=action&view=table");
    act(() => result.current.close());
    expect(window.location.search).toBe("?view=table");
  });
});

describe("useRecordRoute, a hash that is no record", () => {
  it("stays closed for the anchor of the skip link", () => {
    expect(mount("/events/e1/work#main").result.current.route).toEqual({ mode: "closed" });
  });

  it("stays closed for a hash without the shape of a readable ID", () => {
    expect(mount("/events/e1/work#ACT-").result.current.route).toEqual({ mode: "closed" });
    expect(mount("/events/e1/work#012").result.current.route).toEqual({ mode: "closed" });
  });
});
