import { render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { Route, Router, Routes } from "../router/Router";
import { LEGACY_EVENT_PAGES, LegacyEventRedirect } from "./LegacyEventRedirect";

const EVENT = "0199b8e0-0000-7000-8000-0000000000e1";

function renderAt(path: string) {
  window.history.replaceState(null, "", "/start");
  window.history.pushState(null, "", path);
  return render(
    <Router>
      <Routes>
        {LEGACY_EVENT_PAGES.map((page) => (
          <Route key={page} path={`/events/:eventId/${page}`}>
            <LegacyEventRedirect page={page} />
          </Route>
        ))}
        <Route path="*">
          <p>Ziel</p>
        </Route>
      </Routes>
    </Router>,
  );
}

const at = () => window.location.pathname + window.location.search + window.location.hash;

afterEach(() => window.history.replaceState(null, "", "/"));

describe("LegacyEventRedirect", () => {
  it.each([
    ["actions", `/events/${EVENT}/work?type=action`],
    ["commitments", `/events/${EVENT}/work?type=commitment`],
    ["milestones", `/events/${EVENT}/work?type=milestone`],
    ["decisions", `/events/${EVENT}/work?type=decision`],
    ["timeline", `/events/${EVENT}/work?view=timeline`],
    ["workstreams", `/events/${EVENT}/work`],
    ["members", `/events/${EVENT}#mitglieder`],
  ])("sends the old page %s to its place in the new structure", (page, target) => {
    renderAt(`/events/${EVENT}/${page}`);
    expect(screen.getByText("Ziel")).toBeInTheDocument();
    expect(at()).toBe(target);
  });

  it("keeps the readable ID of a record, so that the panel opens it", () => {
    renderAt(`/events/${EVENT}/actions#ACT-001`);
    expect(at()).toBe(`/events/${EVENT}/work?type=action#ACT-001`);
  });

  it("keeps a hash on a page without a type", () => {
    renderAt(`/events/${EVENT}/workstreams#WS-002`);
    expect(at()).toBe(`/events/${EVENT}/work#WS-002`);
  });

  it("keeps the query keys that it does not know", () => {
    renderAt(`/events/${EVENT}/commitments?pseudo&owner=me#COM-003`);
    const params = new URLSearchParams(window.location.search);
    expect(window.location.pathname).toBe(`/events/${EVENT}/work`);
    expect(params.get("type")).toBe("commitment");
    expect(params.get("owner")).toBe("me");
    expect(params.has("pseudo")).toBe(true);
    expect(window.location.hash).toBe("#COM-003");
  });

  it("keeps the query of a link to the members", () => {
    renderAt(`/events/${EVENT}/members?pseudo`);
    expect(window.location.pathname).toBe(`/events/${EVENT}`);
    expect(new URLSearchParams(window.location.search).has("pseudo")).toBe(true);
    expect(window.location.hash).toBe("#mitglieder");
  });

  it("lets the page decide the type, not an old query", () => {
    renderAt(`/events/${EVENT}/milestones?type=action`);
    expect(new URLSearchParams(window.location.search).getAll("type")).toEqual(["milestone"]);
  });

  it("keeps an escaped event ID escaped", () => {
    renderAt("/events/a%20b/actions");
    expect(at()).toBe("/events/a%20b/work?type=action");
  });

  it("replaces the history entry, so that Back leaves the old address behind", async () => {
    renderAt(`/events/${EVENT}/actions`);
    expect(window.location.pathname).toBe(`/events/${EVENT}/work`);
    window.history.back();
    await waitFor(() => expect(window.location.pathname).toBe("/start"));
  });
});
