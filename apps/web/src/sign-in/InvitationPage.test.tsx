import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { StrictMode } from "react";
import { afterEach, describe, expect, it } from "vitest";
import { createApi } from "../api/client";
import { Router, usePathname } from "../router/Router";
import { SessionProvider } from "../session/SessionProvider";
import { InvitationPage } from "./InvitationPage";
import { fakeApi, findAlert, json, problem, queryAlert, watchAlerts } from "./testing";

const membership = {
  organization_id: "0199b8e0-0000-7000-8000-0000000000a1",
  name: "Fliegergruppe Testwil",
  role: "admin",
};
const session = {
  user_id: "0199b8e0-0000-7000-8000-0000000000b1",
  display_name: "Anna Muster",
  organization: membership,
  memberships: [membership],
};

function Where() {
  return <p data-testid="where">{usePathname()}</p>;
}

function renderAt(hash: string, ...responses: Response[]) {
  window.history.replaceState(null, "", `/invitation${hash}`);
  const { api, calls } = fakeApi(problem(401, "unauthenticated"), ...responses);
  render(
    <Router>
      <SessionProvider api={api}>
        <InvitationPage api={api} />
        <Where />
      </SessionProvider>
    </Router>,
  );
  return calls;
}

afterEach(() => window.history.replaceState(null, "", "/"));

describe("InvitationPage", () => {
  it("previews the invitation with the token in the body and removes the fragment", async () => {
    const calls = renderAt(
      "#token=invite-token",
      json(200, { organization_name: "Fliegergruppe Testwil", role: "admin" }),
    );
    expect(await screen.findByText(/Fliegergruppe Testwil/)).toBeInTheDocument();
    expect(screen.getByText(/Administration/)).toBeInTheDocument();
    expect(window.location.hash).toBe("");
    expect(calls[1]).toEqual({
      method: "POST",
      path: "/api/v1/invitations/preview",
      body: JSON.stringify({ token: "invite-token" }),
    });
  });

  it("accepts only after the click, then opens the events", async () => {
    const calls = renderAt(
      "#token=invite-token",
      json(200, { organization_name: "Fliegergruppe Testwil", role: "member" }),
      json(200, session),
      json(200, session),
    );
    const button = await screen.findByRole("button", { name: "Einladung annehmen" });
    expect(calls.some((call) => call.path.endsWith("/accept"))).toBe(false);
    await userEvent.click(button);

    expect(await screen.findByText("/events")).toBeInTheDocument();
    expect(calls.find((call) => call.path.endsWith("/accept"))?.body).toBe(
      JSON.stringify({ token: "invite-token" }),
    );
  });

  it("does not call a rate-limited accept invalid, and the button keeps focus", async () => {
    renderAt(
      "#token=invite-token",
      json(200, { organization_name: "Fliegergruppe Testwil", role: "owner" }),
      problem(429, "rate-limited", { "Retry-After": "1" }),
    );
    expect(await screen.findByText(/Organisationsleitung/)).toBeInTheDocument();
    const button = screen.getByRole("button", { name: "Einladung annehmen" });
    await userEvent.click(button);

    const alert = await findAlert();
    expect(alert).toHaveTextContent("Zu viele Anfragen. Versuchen Sie es in 1 Sekunde erneut.");
    expect(alert).not.toHaveTextContent("ungültig");
    expect(button).toHaveFocus();
    await waitFor(() => expect(button).not.toHaveAttribute("aria-disabled"));
  });

  it("does not call a rate-limited preview invalid and offers a retry", async () => {
    renderAt(
      "#token=invite-token",
      problem(429, "rate-limited"),
      json(200, { organization_name: "Fliegergruppe Testwil", role: "member" }),
    );
    const alert = await findAlert();
    expect(alert).not.toHaveTextContent("ungültig");
    await userEvent.click(screen.getByRole("button", { name: "Erneut versuchen" }));
    expect(await screen.findByText(/Fliegergruppe Testwil/)).toBeInTheDocument();
    // The retry button left with the message: focus goes to the heading.
    await waitFor(() => expect(screen.getByRole("heading", { name: "Einladung" })).toHaveFocus());
  });

  it("moves focus to the message when a retry of the preview fails", async () => {
    renderAt("#token=invite-token", problem(503, "unavailable"), problem(503, "unavailable"));
    await findAlert();
    await userEvent.click(screen.getByRole("button", { name: "Erneut versuchen" }));

    await waitFor(() => expect(queryAlert()).toHaveFocus());
  });

  it("announces a second identical failure of the accept while the button stays", async () => {
    renderAt(
      "#token=invite-token",
      json(200, { organization_name: "Fliegergruppe Testwil", role: "member" }),
      problem(503, "unavailable"),
      problem(503, "unavailable"),
    );
    const button = await screen.findByRole("button", { name: "Einladung annehmen" });
    const alerts = watchAlerts();
    await userEvent.click(button);
    await findAlert();
    await userEvent.click(button);
    await findAlert();
    expect(alerts.stop()).toBe(2);
    expect(button).toHaveFocus();
  });

  it("ignores a stale answer of the preview that arrives after the newer one", async () => {
    window.history.replaceState(null, "", "/invitation#token=invite-token");
    const pending: ((response: Response) => void)[] = [];
    const fetch = async (request: Request) => {
      if (new URL(request.url).pathname === "/api/v1/session") {
        return problem(401, "unauthenticated");
      }
      return new Promise<Response>((resolve) => pending.push(resolve));
    };
    const api = createApi(fetch as unknown as typeof globalThis.fetch);
    render(
      <StrictMode>
        <Router>
          <SessionProvider api={api}>
            <InvitationPage api={api} />
          </SessionProvider>
        </Router>
      </StrictMode>,
    );
    await waitFor(() => expect(pending).toHaveLength(2));
    await act(async () => {
      pending[1]?.(json(200, { organization_name: "Fliegergruppe Testwil", role: "member" }));
    });
    expect(await screen.findByText(/Fliegergruppe Testwil/)).toBeInTheDocument();
    await act(async () => {
      pending[0]?.(problem(503, "unavailable"));
    });
    expect(queryAlert()).not.toBeInTheDocument();
  });

  it("moves focus to the message when the accept fails for good", async () => {
    renderAt(
      "#token=invite-token",
      json(200, { organization_name: "Fliegergruppe Testwil", role: "member" }),
      problem(401, "unauthenticated"),
    );
    await userEvent.click(await screen.findByRole("button", { name: "Einladung annehmen" }));
    const alert = await findAlert();
    expect(alert).toHaveTextContent("Diese Einladung ist ungültig oder abgelaufen.");
    expect(alert).toHaveFocus();
  });

  it("shows the invalid-invitation message for a rejected token", async () => {
    renderAt("#token=old", problem(401, "unauthenticated"));
    expect(await findAlert()).toHaveTextContent("Diese Einladung ist ungültig oder abgelaufen.");
    expect(screen.getByRole("link", { name: "Zur Anmeldung" })).toHaveAttribute("href", "/sign-in");
  });
});
