import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";
import type { Event } from "../api/client";
import { EVENT, ME, problem, renderWork } from "../test/fakeWorkServer";
import { ActionsPage } from "./ActionsPage";
import { CommitmentsPage } from "./CommitmentsPage";
import { WorkstreamsPage } from "./WorkstreamsPage";

const user = userEvent.setup({ delay: null });

const CLOSED: Event = { ...EVENT, status: "closed", closed_at: "2030-07-01T10:00:00Z" };

const action = {
  id: "a1",
  local_id: "ACT-001",
  event_id: EVENT.id,
  title: "Generator bestellen",
  owner_user_id: ME,
  workstream_id: "w1",
  due_date: "2030-05-18",
  status: "open",
  version: 2,
  can_change: true,
  next_statuses: ["in-progress"],
};

const commitment = {
  id: "c1",
  local_id: "COM-001",
  event_id: EVENT.id,
  text: "Zelt liefern",
  owner_user_id: ME,
  condition: "subject to signed order",
  promisor: { kind: "institution", id: "i1", local_id: "INS-001", name: "Generatoren AG" },
  workstream_id: "w1",
  due_date: "2030-05-18",
  status: "conditional",
  version: 2,
  can_change: true,
  can_make_firm: true,
  next_statuses: [],
  evidence: [],
  redacted: false,
};

afterEach(() => window.history.replaceState(null, "", "/"));

describe("the registers of a closed event", () => {
  it("actions: hides the edit control and the form", async () => {
    renderWork((api) => <ActionsPage api={api} />, {
      event: CLOSED,
      lists: { "/actions": [action] },
    });

    await screen.findByRole("table", { name: "Aufgaben" });
    expect(screen.getByText("Generator bestellen")).toBeVisible();
    expect(screen.queryByRole("button", { name: "ACT-001 bearbeiten" })).not.toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "Aufgabe erfassen" })).not.toBeInTheDocument();
  });

  it("commitments: hides edit, make firm and the form", async () => {
    renderWork((api) => <CommitmentsPage api={api} />, {
      event: CLOSED,
      lists: { "/commitments": [commitment] },
    });

    await screen.findByRole("table", { name: "Zusagen" });
    expect(screen.queryByRole("button", { name: "COM-001 bearbeiten" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /verbindlich/ })).not.toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "Zusage erfassen" })).not.toBeInTheDocument();
  });

  it("workstreams: hides the edit control and the form for a manager", async () => {
    renderWork((api) => <WorkstreamsPage api={api} />, { event: CLOSED, eventManager: true });

    await screen.findByRole("table", { name: "Arbeitsbereiche" });
    expect(screen.queryByRole("button", { name: /bearbeiten/ })).not.toBeInTheDocument();
    expect(
      screen.queryByRole("heading", { name: "Arbeitsbereich anlegen" }),
    ).not.toBeInTheDocument();
  });

  it("an active event keeps the controls", async () => {
    renderWork((api) => <ActionsPage api={api} />, { lists: { "/actions": [action] } });

    expect(await screen.findByRole("button", { name: "ACT-001 bearbeiten" })).toBeVisible();
  });

  it("server 409 event-closed shows message", async () => {
    // The page loaded before another member closed the event: the server refuses the write.
    renderWork((api) => <ActionsPage api={api} />, {
      lists: { "/actions": [action] },
      answers: { "PATCH /actions/a1": () => problem(409, "event-closed") },
    });

    await user.click(await screen.findByRole("button", { name: "ACT-001 bearbeiten" }));
    await user.click(await screen.findByRole("button", { name: "Speichern" }));

    // The alert region is always there: wait for the text, not for the region.
    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent("Dieser Anlass ist abgeschlossen"),
    );
  });
});
