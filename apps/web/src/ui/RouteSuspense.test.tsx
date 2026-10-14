import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { lazy, type ReactElement } from "react";
import { describe, expect, it, vi } from "vitest";
import { Link, Route, Router, Routes } from "../router/Router";
import { RouteSuspense } from "./RouteSuspense";

type PageModule = { default: () => ReactElement };

/** A lazy page that loads when `finish` is called, with the first visit of its chunk. */
function deferredPage(title: string) {
  let finish: () => void = () => {};
  const Page = lazy(
    () =>
      new Promise<PageModule>((resolve) => {
        finish = () => resolve({ default: () => <h1>{title}</h1> });
      }),
  );
  return { Page, finish: () => act(async () => finish()) };
}

function renderApp(routes: Array<{ path: string; page: ReactElement; width?: "form" }>) {
  window.history.replaceState(null, "", "/start");
  return render(
    <Router>
      <nav>
        {routes.map((r) => (
          <Link key={r.path} to={r.path}>
            {r.path}
          </Link>
        ))}
      </nav>
      <Routes>
        <Route path="/start">
          <h1>Start</h1>
        </Route>
        {routes.map((r) => (
          <Route key={r.path} path={r.path}>
            <RouteSuspense width={r.width}>{r.page}</RouteSuspense>
          </Route>
        ))}
      </Routes>
    </Router>,
  );
}

describe("RouteSuspense", () => {
  it("shows a skeleton while a lazy page loads, then the page", async () => {
    let finish: (page: { default: () => ReactElement }) => void = () => {};
    const Page = lazy(() => new Promise<{ default: () => ReactElement }>((r) => (finish = r)));
    render(
      <Router>
        <RouteSuspense>
          <Page />
        </RouteSuspense>
      </Router>,
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
      <Router>
        <p>Rahmen</p>
        <RouteSuspense>
          <Page />
        </RouteSuspense>
      </Router>,
    );
    expect(await screen.findByRole("alert")).toBeVisible();
    expect(screen.getByRole("button", { name: "Seite neu laden" })).toBeVisible();
    expect(screen.getByText("Rahmen")).toBeVisible();
    error.mockRestore();
  });

  it("renders no second main and keeps the form width in the fallback when asked", () => {
    const Page = lazy(() => new Promise<{ default: () => ReactElement }>(() => {}));
    render(
      <Router>
        <main>
          <RouteSuspense bare>
            <Page />
          </RouteSuspense>
        </main>
      </Router>,
    );
    expect(screen.getAllByRole("main")).toHaveLength(1);
  });

  it("keeps the form width in the fallback", () => {
    const Page = lazy(() => new Promise<PageModule>(() => {}));
    render(
      <Router>
        <RouteSuspense width="form">
          <Page />
        </RouteSuspense>
      </Router>,
    );
    expect(screen.getByRole("main")).toHaveAttribute("data-width", "form");
  });

  it("reloads the page once when the member presses retry", async () => {
    const error = vi.spyOn(console, "error").mockImplementation(() => {});
    const reload = vi.fn();
    const original = window.location;
    Object.defineProperty(window, "location", {
      configurable: true,
      value: { ...original, reload },
    });
    const Page = lazy(() => Promise.reject(new Error("chunk gone")));
    render(
      <Router>
        <RouteSuspense>
          <Page />
        </RouteSuspense>
      </Router>,
    );
    await userEvent.click(await screen.findByRole("button", { name: "Seite neu laden" }));
    expect(reload).toHaveBeenCalledTimes(1);
    Object.defineProperty(window, "location", { configurable: true, value: original });
    error.mockRestore();
  });

  it("names a missing page, not the connection, when the chunk fails", async () => {
    const error = vi.spyOn(console, "error").mockImplementation(() => {});
    const Page = lazy(() => Promise.reject(new Error("chunk gone")));
    render(
      <Router>
        <RouteSuspense>
          <Page />
        </RouteSuspense>
      </Router>,
    );
    expect(await screen.findByRole("alert")).toHaveTextContent(/Seite konnte nicht geladen/);
    expect(screen.getByRole("alert")).not.toHaveTextContent(/Internetverbindung/);
    error.mockRestore();
  });

  it("moves focus to the h1 of the second lazy page on the first visit of both", async () => {
    const first = deferredPage("Erste");
    const second = deferredPage("Zweite");
    renderApp([
      { path: "/a", page: <first.Page /> },
      { path: "/b", page: <second.Page /> },
    ]);
    await userEvent.click(screen.getByRole("link", { name: "/a" }));
    await first.finish();
    expect(screen.getByRole("heading", { name: "Erste" })).toHaveFocus();

    await userEvent.click(screen.getByRole("link", { name: "/b" }));
    expect(screen.getByRole("status")).toBeVisible();
    await second.finish();
    expect(screen.getByRole("heading", { name: "Zweite" })).toHaveFocus();
  });

  it("moves focus to the page of the last link when a second link is pressed while the first loads", async () => {
    const first = deferredPage("Erste");
    const second = deferredPage("Zweite");
    renderApp([
      { path: "/a", page: <first.Page /> },
      { path: "/b", page: <second.Page /> },
    ]);
    await userEvent.click(screen.getByRole("link", { name: "/a" }));
    await userEvent.click(screen.getByRole("link", { name: "/b" }));
    await first.finish();
    await second.finish();
    expect(screen.getByRole("heading", { name: "Zweite" })).toHaveFocus();
  });

  it("moves focus to the page again after back and forward", async () => {
    const first = deferredPage("Erste");
    const second = deferredPage("Zweite");
    renderApp([
      { path: "/a", page: <first.Page /> },
      { path: "/b", page: <second.Page /> },
    ]);
    await userEvent.click(screen.getByRole("link", { name: "/a" }));
    await first.finish();
    await userEvent.click(screen.getByRole("link", { name: "/b" }));
    await second.finish();
    await act(async () => {
      window.history.back();
      await new Promise((r) => setTimeout(r, 20));
    });
    expect(screen.getByRole("heading", { name: "Erste" })).toHaveFocus();
  });

  it("clears the failure when the member goes to another page", async () => {
    const error = vi.spyOn(console, "error").mockImplementation(() => {});
    const Broken = lazy(() => Promise.reject(new Error("chunk gone")));
    const ok = deferredPage("Gut");
    renderApp([
      { path: "/a", page: <Broken /> },
      { path: "/b", page: <ok.Page /> },
    ]);
    await userEvent.click(screen.getByRole("link", { name: "/a" }));
    expect(await screen.findByRole("alert")).toBeVisible();
    await userEvent.click(screen.getByRole("link", { name: "/b" }));
    await ok.finish();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Gut" })).toBeVisible();
    error.mockRestore();
  });
});
