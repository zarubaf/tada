import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";
import { Route, Router, Routes, usePathname } from "../router/Router";
import { fakeApi, findAlert, json, problem } from "../sign-in/testing";
import { ChooseOrganizationPage } from "./ChooseOrganizationPage";
import { SessionProvider } from "./SessionProvider";

const first = {
  organization_id: "0199b8e0-0000-7000-8000-0000000000a1",
  name: "Fliegergruppe Testwil",
  role: "member",
};
const second = {
  organization_id: "0199b8e0-0000-7000-8000-0000000000a2",
  name: "Segelflugclub Musterhausen",
  role: "admin",
};
const withoutOrganization = {
  user_id: "0199b8e0-0000-7000-8000-0000000000b1",
  display_name: "Anna Muster",
  memberships: [first, second],
};

function Where() {
  return <p data-testid="where">{usePathname()}</p>;
}

function renderPage(...responses: Response[]) {
  window.history.replaceState(null, "", "/choose-organization");
  const { api, calls } = fakeApi(json(200, withoutOrganization), ...responses);
  render(
    <Router>
      <SessionProvider api={api}>
        <Routes>
          <Route path="/choose-organization">
            <ChooseOrganizationPage api={api} />
          </Route>
        </Routes>
        <Where />
      </SessionProvider>
    </Router>,
  );
  return calls;
}

afterEach(() => window.history.replaceState(null, "", "/"));

describe("ChooseOrganizationPage", () => {
  it("shows one button for each membership", async () => {
    renderPage();
    expect(
      await screen.findByRole("button", { name: /Fliegergruppe Testwil/ }),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Segelflugclub Musterhausen/ })).toBeInTheDocument();
  });

  it("chooses the organization, loads the session again and opens the events", async () => {
    const chosen = { ...withoutOrganization, organization: second };
    const calls = renderPage(json(200, chosen), json(200, chosen));
    await userEvent.click(
      await screen.findByRole("button", { name: /Segelflugclub Musterhausen/ }),
    );

    expect(await screen.findByText("/events")).toBeInTheDocument();
    expect(calls[1]).toEqual({
      method: "POST",
      path: "/api/v1/session/organization",
      body: JSON.stringify({ organization_id: second.organization_id }),
    });
  });

  it("signs out, because the page has no shell", async () => {
    const calls = renderPage(new Response(null, { status: 204 }));
    await userEvent.click(await screen.findByRole("button", { name: "Abmelden" }));

    expect(await screen.findByText("/sign-in")).toBeInTheDocument();
    expect(calls[1]).toMatchObject({ method: "POST", path: "/api/v1/sign-out" });
  });

  it("shows the message of the problem and stays on the page", async () => {
    renderPage(problem(403, "forbidden"));
    const choice = await screen.findByRole("button", { name: /Fliegergruppe Testwil/ });
    await userEvent.click(choice);

    expect(await findAlert()).toHaveTextContent("Sie haben keine Berechtigung für diese Aktion.");
    expect(screen.getByTestId("where")).toHaveTextContent("/choose-organization");
    // The buttons stay, so the pressed one keeps focus.
    expect(choice).toHaveFocus();
  });
});
