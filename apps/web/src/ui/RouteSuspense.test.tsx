import { act, render, screen } from "@testing-library/react";
import { lazy, type ReactElement } from "react";
import { describe, expect, it } from "vitest";
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
});
