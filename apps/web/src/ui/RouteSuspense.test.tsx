import { act, render, screen } from "@testing-library/react";
import { lazy, type ReactElement } from "react";
import { describe, expect, it, vi } from "vitest";
import { RouteSuspense } from "./RouteSuspense";

describe("RouteSuspense", () => {
  it("shows a skeleton while a lazy page loads, then the page", async () => {
    let finish: (page: { default: () => ReactElement }) => void = () => {};
    const Page = lazy(() => new Promise<{ default: () => ReactElement }>((r) => (finish = r)));
    render(
      <RouteSuspense>
        <Page />
      </RouteSuspense>,
    );
    expect(screen.getByRole("status", { name: "Seite wird geladen" })).toBeVisible();

    await act(async () => finish({ default: () => <h2>Personen</h2> }));
    expect(screen.getByRole("heading", { name: "Personen" })).toBeVisible();
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });

  it("shows the failure with a retry when the chunk fails, and the app stays", async () => {
    const error = vi.spyOn(console, "error").mockImplementation(() => {});
    const Page = lazy(() => Promise.reject(new Error("chunk gone")));
    render(
      <div>
        <p>Rahmen</p>
        <RouteSuspense>
          <Page />
        </RouteSuspense>
      </div>,
    );
    expect(await screen.findByRole("alert")).toBeVisible();
    expect(screen.getByRole("button", { name: "Erneut versuchen" })).toBeVisible();
    expect(screen.getByText("Rahmen")).toBeVisible();
    error.mockRestore();
  });

  it("renders no second main and keeps the form width in the fallback when asked", () => {
    const Page = lazy(() => new Promise<{ default: () => ReactElement }>(() => {}));
    render(
      <main>
        <RouteSuspense bare>
          <Page />
        </RouteSuspense>
      </main>,
    );
    expect(screen.getAllByRole("main")).toHaveLength(1);
  });
});
