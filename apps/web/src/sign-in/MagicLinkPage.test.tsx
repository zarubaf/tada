import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";
import { Router, usePathname } from "../router/Router";
import { SessionProvider } from "../session/SessionProvider";
import { MagicLinkPage } from "./MagicLinkPage";
import { fakeApi, json, problem } from "./testing";

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

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Dieser Link ist ungültig oder abgelaufen.",
    );
    expect(screen.getByRole("link", { name: "Zur Anmeldung" })).toHaveAttribute("href", "/sign-in");
  });

  it("keeps the token when the session of a signed-in member loads after the first render", async () => {
    renderWith(json(200, session), "#token=secret-token");
    await screen.findByRole("button", { name: "Anmelden" });
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(screen.getByRole("button", { name: "Anmelden" })).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("shows the invalid-link message without a token", async () => {
    renderAt("");
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Dieser Link ist ungültig oder abgelaufen.",
    );
    expect(screen.queryByRole("button", { name: "Anmelden" })).not.toBeInTheDocument();
  });
});
