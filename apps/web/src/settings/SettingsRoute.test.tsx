import { act, render, screen } from "@testing-library/react";
import { lazy, type ReactElement } from "react";
import { afterEach, describe, expect, it } from "vitest";
import { Router } from "../router/Router";
import { SettingsRoute } from "./SettingsRoute";

afterEach(() => window.history.replaceState(null, "", "/"));

describe("SettingsRoute", () => {
  it("shows a loading state and the sub-navigation while the page loads, then the page", async () => {
    window.history.replaceState(null, "", "/settings/account");
    let finish: (page: { default: () => ReactElement }) => void = () => {};
    const Page = lazy(() => new Promise<{ default: () => ReactElement }>((r) => (finish = r)));
    render(
      <Router>
        <SettingsRoute>
          <Page />
        </SettingsRoute>
      </Router>,
    );

    expect(screen.getByRole("status", { name: "Einstellungen werden geladen" })).toBeVisible();
    expect(screen.getByRole("navigation", { name: "Einstellungen" })).toBeVisible();

    await act(async () => finish({ default: () => <h2>Konto</h2> }));
    expect(screen.getByRole("heading", { name: "Konto" })).toBeVisible();
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });
});
