import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useEffect, useState } from "react";
import { afterEach, describe, expect, it } from "vitest";
import {
  Link,
  Redirect,
  Route,
  Router,
  Routes,
  useHash,
  useNavigate,
  useParams,
  useSearch,
  useSetSearch,
  useSettleFocus,
} from "./Router";

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

  it("reads the hash of the address and follows its changes", async () => {
    function Hash() {
      return <p>Hash: {useHash()}</p>;
    }
    window.history.replaceState(null, "", "/events#ACT-002");
    render(
      <Router>
        <Link to="/events#ACT-003">Weiter</Link>
        <Hash />
      </Router>,
    );
    expect(screen.getByText("Hash: ACT-002")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("link", { name: "Weiter" }));
    expect(screen.getByText("Hash: ACT-003")).toBeInTheDocument();
    expect(window.location.hash).toBe("#ACT-003");
    window.history.replaceState(null, "", "/events");
    fireEvent.popState(window);
    expect(screen.getByText("Hash:")).toBeInTheDocument();
  });

  describe("query string", () => {
    function Filters() {
      const search = useSearch();
      const setSearch = useSetSearch();
      return (
        <>
          <p>
            Filter: {search.get("type") ?? "none"} / {search.getAll("tag").join(",")} /{" "}
            {search.has("view") ? "view" : "no view"}
          </p>
          <button type="button" onClick={() => setSearch({ type: "talk" }, { replace: true })}>
            filter
          </button>
          <button type="button" onClick={() => setSearch({ view: "board" })}>
            tab
          </button>
          <button type="button" onClick={() => setSearch({ type: null, view: undefined })}>
            clear
          </button>
          <button type="button" onClick={() => setSearch({ view: "board" })}>
            board
          </button>
          <button type="button" onClick={() => setSearch({ tag: ["a", "b"] })}>
            tags
          </button>
        </>
      );
    }

    function renderFilters(path: string) {
      window.history.replaceState(null, "", path);
      return render(
        <Router>
          <Link to="/events?type=party">Party</Link>
          <Link to="/events">Anlässe</Link>
          <Filters />
          <Routes>
            <Route path="/events">
              <h1>Liste</h1>
            </Route>
            <Route path="/other">
              <h1>Andere Seite</h1>
            </Route>
          </Routes>
        </Router>,
      );
    }

    it("reads the parameters of the address", () => {
      renderFilters("/events?type=party&tag=x&tag=y&view=");
      expect(screen.getByText("Filter: party / x,y / view")).toBeInTheDocument();
    });

    it("re-renders consumers when only the query changes", async () => {
      renderFilters("/events?type=party");
      await userEvent.click(screen.getByRole("button", { name: "filter" }));
      expect(screen.getByText(/Filter: talk/)).toBeInTheDocument();
      expect(window.location.search).toBe("?type=talk");
    });

    it("keeps the path and the hash when it sets parameters", async () => {
      renderFilters("/events?type=party#ACT-012");
      await userEvent.click(screen.getByRole("button", { name: "tab" }));
      expect(window.location.pathname).toBe("/events");
      expect(window.location.search).toBe("?type=party&view=board");
      expect(window.location.hash).toBe("#ACT-012");
    });

    it("replaces or pushes the history entry as the caller chooses", async () => {
      renderFilters("/events");
      const length = window.history.length;
      await userEvent.click(screen.getByRole("button", { name: "filter" }));
      expect(window.history.length).toBe(length);
      await userEvent.click(screen.getByRole("button", { name: "tab" }));
      expect(window.history.length).toBe(length + 1);
    });

    it("removes parameters set to null or undefined and sets repeated values", async () => {
      renderFilters("/events?type=party&view=board");
      await userEvent.click(screen.getByRole("button", { name: "clear" }));
      expect(window.location.search).toBe("");
      await userEvent.click(screen.getByRole("button", { name: "tags" }));
      expect(window.location.search).toBe("?tag=a&tag=b");
    });

    it("follows back and forward across query changes", async () => {
      renderFilters("/events");
      await userEvent.click(screen.getByRole("button", { name: "tab" }));
      await userEvent.click(screen.getByRole("link", { name: "Party" }));
      expect(screen.getByText(/Filter: party/)).toBeInTheDocument();
      window.history.back();
      expect(await screen.findByText(/Filter: none \/ \/ view/)).toBeInTheDocument();
      window.history.back();
      expect(await screen.findByText(/Filter: none \/ \/ no view/)).toBeInTheDocument();
      window.history.forward();
      expect(await screen.findByText(/Filter: none \/ \/ view/)).toBeInTheDocument();
    });

    it("does not push an entry when the whole address is equal", async () => {
      renderFilters("/events?type=party#ACT-012");
      const length = window.history.length;
      function Same() {
        const navigate = useNavigate();
        return (
          <button type="button" onClick={() => navigate("/events?type=party#ACT-012")}>
            same
          </button>
        );
      }
      render(
        <Router>
          <Same />
        </Router>,
      );
      await userEvent.click(screen.getByRole("button", { name: "same" }));
      expect(window.history.length).toBe(length);
    });

    it("handles a malformed or unknown query without throwing", () => {
      renderFilters("/events?%E0%A4%A=1&type=%E0%A4%A&&=&unknown");
      expect(screen.getByText(/Filter:/)).toBeInTheDocument();
      expect(screen.getByRole("heading", { name: "Liste" })).toBeInTheDocument();
    });

    it("marks a link current by its path alone, whatever the query", () => {
      renderFilters("/events?type=talk");
      expect(screen.getByRole("link", { name: "Party" })).toHaveAttribute("aria-current", "page");
      expect(screen.getByRole("link", { name: "Anlässe" })).toHaveAttribute("aria-current", "page");
    });

    it("does not mark a link current on a path that another link claims", () => {
      window.history.replaceState(null, "", "/events/e1/work");
      render(
        <Router>
          <Link to="/events" yieldsTo="/events/e1/work">
            Anlässe
          </Link>
          <Link to="/events/e1/work">Arbeit</Link>
        </Router>,
      );
      expect(screen.getByRole("link", { name: "Anlässe" })).not.toHaveAttribute("aria-current");
      expect(screen.getByRole("link", { name: "Arbeit" })).toHaveAttribute("aria-current", "page");
    });

    it("does not mark a link current on any of several paths that other links claim", () => {
      window.history.replaceState(null, "", "/events/e1/documents");
      render(
        <Router>
          <Link to="/events" yieldsTo={["/events/e1/work", "/events/e1/documents"]}>
            Anlässe
          </Link>
          <Link to="/events/e1/documents">Dokumente</Link>
        </Router>,
      );
      expect(screen.getByRole("link", { name: "Anlässe" })).not.toHaveAttribute("aria-current");
      expect(screen.getByRole("link", { name: "Dokumente" })).toHaveAttribute(
        "aria-current",
        "page",
      );
    });

    it("marks a link current only for its query value, and its sibling not then", () => {
      window.history.replaceState(null, "", "/events/e1/work?view=timeline&q=x");
      render(
        <Router>
          <Link to="/events/e1/work" unlessWith={{ name: "view", value: "timeline" }}>
            Arbeit
          </Link>
          <Link to="/events/e1/work?view=timeline" onlyWith={{ name: "view", value: "timeline" }}>
            Zeitplan
          </Link>
        </Router>,
      );
      expect(screen.getByRole("link", { name: "Arbeit" })).not.toHaveAttribute("aria-current");
      expect(screen.getByRole("link", { name: "Zeitplan" })).toHaveAttribute(
        "aria-current",
        "page",
      );
    });

    it("keeps a link with a query rule not current on the page without that query", () => {
      window.history.replaceState(null, "", "/events/e1/work?view=table");
      render(
        <Router>
          <Link to="/events/e1/work" unlessWith={{ name: "view", value: "timeline" }}>
            Arbeit
          </Link>
          <Link to="/events/e1/work?view=timeline" onlyWith={{ name: "view", value: "timeline" }}>
            Zeitplan
          </Link>
        </Router>,
      );
      expect(screen.getByRole("link", { name: "Arbeit" })).toHaveAttribute("aria-current", "page");
      expect(screen.getByRole("link", { name: "Zeitplan" })).not.toHaveAttribute("aria-current");
    });

    it("never marks a link without a path current", () => {
      window.history.replaceState(null, "", "/events");
      render(
        <Router>
          <Link to="?view=board">Query</Link>
          <Link to="#ACT-001">Hash</Link>
        </Router>,
      );
      expect(screen.getByRole("link", { name: "Query" })).not.toHaveAttribute("aria-current");
      expect(screen.getByRole("link", { name: "Hash" })).not.toHaveAttribute("aria-current");
    });

    it("keeps focus where it is on a query-only change", async () => {
      renderFilters("/events");
      const button = screen.getByRole("button", { name: "filter" });
      await userEvent.click(button);
      expect(button).toHaveFocus();
      await userEvent.click(screen.getByRole("link", { name: "Party" }));
      expect(screen.getByRole("link", { name: "Party" })).toHaveFocus();
    });

    it("does not leave a focus request behind for a later page", async () => {
      function Settler() {
        const settle = useSettleFocus();
        useEffect(settle, [settle]);
        return null;
      }
      function Later() {
        const navigate = useNavigate();
        const [mounted, setMounted] = useState(false);
        return (
          <>
            <button type="button" onClick={() => navigate("/events?type=talk")}>
              query
            </button>
            <button type="button" onClick={() => setMounted(true)}>
              mount
            </button>
            {mounted ? <Settler /> : null}
          </>
        );
      }
      window.history.replaceState(null, "", "/events");
      render(
        <Router>
          <Later />
          <Routes>
            <Route path="/events">
              <h1>Liste</h1>
            </Route>
          </Routes>
        </Router>,
      );
      await userEvent.click(screen.getByRole("button", { name: "query" }));
      await userEvent.click(screen.getByRole("button", { name: "mount" }));
      expect(screen.getByRole("heading", { name: "Liste" })).not.toHaveFocus();
    });

    it("keeps a waiting focus request when only the query changes", async () => {
      function Lazy() {
        const settle = useSettleFocus();
        const [loaded, setLoaded] = useState(false);
        useEffect(() => {
          if (loaded) {
            settle();
          }
        }, [loaded, settle]);
        return (
          <>
            <button type="button" onClick={() => setLoaded(true)}>
              load
            </button>
            {loaded ? <h1>Lazy</h1> : <p data-route-pending>laden</p>}
          </>
        );
      }
      function Nav() {
        const navigate = useNavigate();
        const setSearch = useSetSearch();
        return (
          <>
            <button type="button" onClick={() => navigate("/lazy")}>
              to-lazy
            </button>
            <button type="button" onClick={() => setSearch({ type: "talk" })}>
              write
            </button>
          </>
        );
      }
      window.history.replaceState(null, "", "/events");
      render(
        <Router>
          <Nav />
          <Routes>
            <Route path="/events">
              <h1>Liste</h1>
            </Route>
            <Route path="/lazy">
              <Lazy />
            </Route>
          </Routes>
        </Router>,
      );
      await userEvent.click(screen.getByRole("button", { name: "to-lazy" }));
      await userEvent.click(screen.getByRole("button", { name: "write" }));
      await userEvent.click(screen.getByRole("button", { name: "load" }));
      expect(screen.getByRole("heading", { name: "Lazy" })).toHaveFocus();
    });

    it("does not push an entry when the active tab is clicked on an unnormalized query", async () => {
      renderFilters("/events?type=a%20b&view=board");
      const length = window.history.length;
      await userEvent.click(screen.getByRole("button", { name: "board" }));
      expect(window.history.length).toBe(length);
      expect(window.location.search).toBe("?type=a%20b&view=board");
    });

    it("does not push an entry when the written key only moves to the end of the query", async () => {
      renderFilters("/events?view=board&type=talk");
      const length = window.history.length;
      await userEvent.click(screen.getByRole("button", { name: "board" }));
      expect(window.history.length).toBe(length);
    });

    it("still moves focus when the path changes together with the query", async () => {
      function ToOther() {
        const navigate = useNavigate();
        return (
          <button type="button" onClick={() => navigate("/other?type=talk")}>
            other
          </button>
        );
      }
      window.history.replaceState(null, "", "/events?type=talk");
      render(
        <Router>
          <ToOther />
          <Routes>
            <Route path="/events">
              <h1>Liste</h1>
            </Route>
            <Route path="/other">
              <h1>Andere Seite</h1>
            </Route>
          </Routes>
        </Router>,
      );
      await userEvent.click(screen.getByRole("button", { name: "other" }));
      expect(screen.getByRole("heading", { name: "Andere Seite" })).toHaveFocus();
    });
  });
});

describe("Link onNavigate", () => {
  it("runs before the path changes, and not for a click with a modifier key", async () => {
    window.history.replaceState(null, "", "/events");
    const seen: string[] = [];
    render(
      <Router>
        <Link to="/other" onNavigate={() => seen.push(window.location.pathname)}>
          Andere
        </Link>
      </Router>,
    );

    fireEvent.click(screen.getByRole("link", { name: "Andere" }), { ctrlKey: true });
    expect(seen).toEqual([]);
    fireEvent.click(screen.getByRole("link", { name: "Andere" }));

    expect(seen).toEqual(["/events"]);
    expect(window.location.pathname).toBe("/other");
  });
});
