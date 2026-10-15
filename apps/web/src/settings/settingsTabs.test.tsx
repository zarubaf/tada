import { act, renderHook } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it } from "vitest";
import { Router } from "../router/Router";
import { redirectTarget, resolveTab, useSettingsTab, visibleTabs } from "./settingsTabs";

afterEach(() => window.history.replaceState(null, "", "/"));

function wrapper({ children }: { children: ReactNode }) {
  return <Router>{children}</Router>;
}

describe("visibleTabs", () => {
  it("shows the AI-PM only to an owner or an admin", () => {
    const all = ["account", "members", "organization", "ai-pm", "telegram", "tokens"];
    expect(visibleTabs("owner")).toEqual(all);
    expect(visibleTabs("admin")).toEqual(all);
    for (const role of ["member", undefined] as const) {
      expect(visibleTabs(role)).toEqual(all.filter((id) => id !== "ai-pm"));
    }
  });
});

describe("resolveTab", () => {
  it("takes a tab that the member sees", () => {
    expect(resolveTab("telegram", "member")).toEqual({ tab: "telegram", denied: false });
  });

  it("falls back to the first tab for no value or an unknown value", () => {
    expect(resolveTab(null, "owner")).toEqual({ tab: "account", denied: false });
    expect(resolveTab("nonsense", "owner")).toEqual({ tab: "account", denied: false });
  });

  it("falls back and reports a tab that the member may not see", () => {
    expect(resolveTab("ai-pm", "member")).toEqual({ tab: "account", denied: true });
  });
});

describe("redirectTarget", () => {
  it("names the tab of each old settings path and keeps the rest", () => {
    expect(redirectTarget("/settings/ai-pm", "", "")).toBe("/settings?tab=ai-pm");
    expect(redirectTarget("/settings/tokens", "?pseudo", "#x")).toBe(
      "/settings?pseudo=&tab=tokens#x",
    );
    expect(redirectTarget("/settings/account", "", "")).toBe("/settings");
  });
});

describe("useSettingsTab", () => {
  it("reads the tab from the query and writes it back, without the default", () => {
    window.history.replaceState(null, "", "/settings?tab=tokens");
    const { result } = renderHook(() => useSettingsTab("member"), { wrapper });
    expect(result.current.tab).toBe("tokens");

    act(() => result.current.select("members"));
    expect(window.location.search).toBe("?tab=members");
    expect(result.current.tab).toBe("members");

    act(() => result.current.select("account"));
    expect(window.location.search).toBe("");
    expect(result.current.tab).toBe("account");
  });

  it("drops a denied tab from the address and reports it once", () => {
    window.history.replaceState(null, "", "/settings?tab=ai-pm");
    const { result } = renderHook(() => useSettingsTab("member"), { wrapper });
    expect(result.current.tab).toBe("account");
    expect(result.current.denied).toBe(true);
    expect(window.location.search).toBe("");

    act(() => result.current.select("telegram"));
    expect(result.current.denied).toBe(false);
  });
});
