import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";
import { Link, Redirect, Route, Router, Routes, useNavigate, useParams } from "./Router";

function Event() {
  const { eventId } = useParams();
  return <h1>Anlass {eventId}</h1>;
}

function Go() {
  const navigate = useNavigate();
  return (
    <>
      <button type="button" onClick={() => navigate("/events/TEST30")}>
        go
      </button>
      <button type="button" onClick={() => navigate("/events/FLY28", { replace: true })}>
        replace
      </button>
    </>
  );
}

function renderAt(path: string) {
  window.history.replaceState(null, "", path);
  return render(
    <Router>
      <nav>
        <Link to="/events">Anlässe</Link>
        <Link to="/other">Andere</Link>
        <Go />
      </nav>
      <Routes>
        <Route path="/">
          <Redirect to="/events" />
        </Route>
        <Route path="/events">
          <h1>Liste</h1>
        </Route>
        <Route path="/events/:eventId">
          <Event />
        </Route>
        <Route path="*">
          <p>Seite nicht gefunden</p>
        </Route>
      </Routes>
    </Router>,
  );
}

afterEach(() => window.history.replaceState(null, "", "/"));

describe("Router", () => {
  it("matches a path with a parameter", () => {
    renderAt("/events/FLY28");
    expect(screen.getByText("Anlass FLY28")).toBeInTheDocument();
    expect(screen.queryByText("Liste")).not.toBeInTheDocument();
  });

  it("shows the not-found route for an unknown path", () => {
    renderAt("/nowhere/at/all");
    expect(screen.getByText("Seite nicht gefunden")).toBeInTheDocument();
  });

  it("redirects and replaces the history entry", () => {
    renderAt("/");
    expect(screen.getByText("Liste")).toBeInTheDocument();
    expect(window.location.pathname).toBe("/events");
  });

  it("marks an exact link only for its own path", () => {
    window.history.replaceState(null, "", "/events/TEST30/members");
    render(
      <Router>
        <Link to="/events/TEST30" exact>
          Übersicht
        </Link>
        <Link to="/events/TEST30/members" exact>
          Mitglieder
        </Link>
      </Router>,
    );
    expect(screen.getByRole("link", { name: "Übersicht" })).not.toHaveAttribute("aria-current");
    expect(screen.getByRole("link", { name: "Mitglieder" })).toHaveAttribute(
      "aria-current",
      "page",
    );
  });

  it("navigates with pushState and marks the current link", async () => {
    renderAt("/events");
    expect(screen.getByRole("link", { name: "Anlässe" })).toHaveAttribute("aria-current", "page");
    expect(screen.getByRole("link", { name: "Andere" })).not.toHaveAttribute("aria-current");

    await userEvent.click(screen.getByRole("link", { name: "Andere" }));
    expect(window.location.pathname).toBe("/other");
    expect(screen.getByText("Seite nicht gefunden")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Andere" })).toHaveAttribute("aria-current", "page");

    await userEvent.click(screen.getByRole("button", { name: "go" }));
    expect(screen.getByText("Anlass TEST30")).toBeInTheDocument();
  });

  it("follows the back button", async () => {
    renderAt("/events");
    await userEvent.click(screen.getByRole("link", { name: "Andere" }));
    window.history.back();
    expect(await screen.findByText("Liste")).toBeInTheDocument();
  });

  it("treats a malformed escape as no match", () => {
    renderAt("/events/%E0%A4%A");
    expect(screen.getByText("Seite nicht gefunden")).toBeInTheDocument();
  });

  it("moves focus to the h1 after a navigation, not after the first render", async () => {
    renderAt("/events");
    expect(document.body).toHaveFocus();

    await userEvent.click(screen.getByRole("button", { name: "go" }));
    expect(screen.getByRole("heading", { name: "Anlass TEST30" })).toHaveFocus();
  });

  it("moves focus to the h1 after a replace that a member caused", async () => {
    renderAt("/events");
    const length = window.history.length;

    await userEvent.click(screen.getByRole("button", { name: "replace" }));
    expect(screen.getByRole("heading", { name: "Anlass FLY28" })).toHaveFocus();
    expect(window.history.length).toBe(length);
  });

  it("leaves focus alone after a redirect", () => {
    const outside = document.createElement("button");
    document.body.append(outside);
    outside.focus();

    renderAt("/");
    expect(screen.getByText("Liste")).toBeInTheDocument();
    expect(outside).toHaveFocus();
    outside.remove();
  });

  it("does not push a duplicate entry for the current path", async () => {
    renderAt("/events");
    const length = window.history.length;
    await userEvent.click(screen.getByRole("link", { name: "Anlässe" }));
    expect(window.history.length).toBe(length);
  });

  it("leaves modified clicks to the browser", async () => {
    renderAt("/events");
    fireEvent.click(screen.getByRole("link", { name: "Andere" }), { altKey: true });
    expect(window.location.pathname).toBe("/events");
  });
});
