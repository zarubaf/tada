import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { StrictMode } from "react";
import { afterEach, describe, expect, it } from "vitest";
import { Router, usePathname } from "../router/Router";
import { SessionProvider } from "../session/SessionProvider";
import { MagicLinkPage } from "./MagicLinkPage";
import { fakeApi, findAlert, json, problem, queryAlert, watchAlerts } from "./testing";

const session = {
  user_id: "0199b8e0-0000-7000-8000-0000000000b1",
  display_name: "Anna Muster",
  memberships: [],
};

function Where() {
  return <p data-testid="where">{usePathname()}</p>;
}

function renderAt(hash: string, ...responses: Response[]) {
  return renderWith(problem(401, "unauthenticated"), hash, ...responses);
}

function renderWith(first: Response, hash: string, ...responses: Response[]) {
  window.history.replaceState(null, "", `/sign-in/link${hash}`);
  const { api, calls } = fakeApi(first, ...responses);
  render(
    <Router>
      <SessionProvider api={api}>
        <MagicLinkPage api={api} />
        <Where />
      </SessionProvider>
    </Router>,
  );
  return calls;
}

afterEach(() => window.history.replaceState(null, "", "/"));

describe("MagicLinkPage", () => {
  it("removes the fragment and does not redeem the token on load", async () => {
    const calls = renderAt("#token=secret-token");
    expect(await screen.findByRole("button", { name: "Anmelden" })).toBeInTheDocument();
    expect(window.location.hash).toBe("");
    expect(window.location.pathname).toBe("/sign-in/link");
    expect(calls.map((call) => call.path)).toEqual(["/api/v1/session"]);
  });

  it("redeems the token in the body only after the click, then leaves the page", async () => {
    const calls = renderAt("#token=secret-token", json(200, session), json(200, session));
    await userEvent.click(await screen.findByRole("button", { name: "Anmelden" }));

    expect(await screen.findByText("/choose-organization")).toBeInTheDocument();
    expect(calls.find((call) => call.path === "/api/v1/sign-in/magic-link")).toEqual({
      method: "POST",
      path: "/api/v1/sign-in/magic-link",
      body: JSON.stringify({ token: "secret-token" }),
    });
  });

  it("shows the invalid-link message with a link to the sign-in page", async () => {
    renderAt("#token=old", problem(401, "unauthenticated"));
    await userEvent.click(await screen.findByRole("button", { name: "Anmelden" }));

    expect(await findAlert()).toHaveTextContent("Dieser Link ist ungültig oder abgelaufen.");
    expect(screen.getByRole("link", { name: "Zur Anmeldung" })).toHaveAttribute("href", "/sign-in");
  });

  it("keeps the token when the session of a signed-in member loads after the first render", async () => {
    const calls = renderWith(json(200, session), "#token=secret-token");
    await screen.findByRole("button", { name: "Anmelden" });
    await waitFor(() => expect(calls).toHaveLength(1));
    await act(async () => {});
    expect(screen.getByRole("button", { name: "Anmelden" })).toBeInTheDocument();
    expect(queryAlert()).not.toBeInTheDocument();
  });

  it("moves focus to the message when the link is invalid", async () => {
    renderAt("#token=old", problem(401, "unauthenticated"));
    await userEvent.click(await screen.findByRole("button", { name: "Anmelden" }));
    expect(await findAlert()).toHaveFocus();
  });

  it("does not call a rate-limited link invalid, and the button keeps focus", async () => {
    renderAt("#token=good", problem(429, "rate-limited", { "Retry-After": "1" }));
    const button = await screen.findByRole("button", { name: "Anmelden" });
    await userEvent.click(button);

    const alert = await findAlert();
    expect(alert).toHaveTextContent("Zu viele Anfragen. Versuchen Sie es in 1 Sekunde erneut.");
    expect(alert).not.toHaveTextContent("ungültig");
    expect(button).toHaveFocus();
    expect(button).toHaveAttribute("aria-disabled", "true");
    await waitFor(() => expect(button).not.toHaveAttribute("aria-disabled"));
    expect(button).toHaveFocus();
  });

  it("announces a second identical failure while the button stays", async () => {
    renderAt("#token=good", problem(503, "unavailable"), problem(503, "unavailable"));
    const button = await screen.findByRole("button", { name: "Anmelden" });
    const alerts = watchAlerts();
    await userEvent.click(button);
    await findAlert();
    await userEvent.click(button);
    await findAlert();
    expect(alerts.stop()).toBe(2);
    expect(button).toHaveFocus();
  });

  it("keeps the token under StrictMode", async () => {
    window.history.replaceState(null, "", "/sign-in/link#token=secret-token");
    const { api } = fakeApi(problem(401, "unauthenticated"));
    render(
      <StrictMode>
        <Router>
          <SessionProvider api={api}>
            <MagicLinkPage api={api} />
          </SessionProvider>
        </Router>
      </StrictMode>,
    );
    expect(await screen.findByRole("button", { name: "Anmelden" })).toBeInTheDocument();
    expect(queryAlert()).not.toBeInTheDocument();
  });

  it("shows the invalid-link message without a token", async () => {
    renderAt("");
    expect(await findAlert()).toHaveTextContent("Dieser Link ist ungültig oder abgelaufen.");
    expect(screen.queryByRole("button", { name: "Anmelden" })).not.toBeInTheDocument();
  });
});
