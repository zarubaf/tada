import { screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import type { Event } from "../api/client";
import { EVENT, renderWork } from "../test/fakeWorkServer";
import { WorkstreamsPage } from "./WorkstreamsPage";

const CLOSED: Event = { ...EVENT, status: "closed", closed_at: "2030-07-01T10:00:00Z" };

afterEach(() => window.history.replaceState(null, "", "/"));

describe("the workstreams of a closed event", () => {
  it("hides the edit control and the form for a manager", async () => {
    renderWork((api) => <WorkstreamsPage api={api} />, { event: CLOSED, eventManager: true });

    await screen.findByRole("table", { name: "Arbeitsbereiche" });
    expect(screen.queryByRole("button", { name: /bearbeiten/ })).not.toBeInTheDocument();
    expect(
      screen.queryByRole("heading", { name: "Arbeitsbereich anlegen" }),
    ).not.toBeInTheDocument();
  });
});
