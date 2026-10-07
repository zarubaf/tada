import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { Router } from "../router/Router";
import { SignInPage } from "./SignInPage";
import { fakeApi, json, problem } from "./testing";

const SAME_MESSAGE = "Wenn die Adresse bekannt ist, erhalten Sie in Kürze eine E-Mail.";

async function submit(address: string, ...responses: Response[]) {
  const { api, calls } = fakeApi(...responses);
  render(
    <Router>
      <SignInPage api={api} />
    </Router>,
  );
  await userEvent.click(screen.getByLabelText(/E-Mail-Adresse/));
  await userEvent.paste(address);
  await userEvent.click(screen.getByRole("button", { name: "Anmeldelink senden" }));
  return calls;
}

describe("SignInPage", () => {
  it("posts the address and shows the same message for each address", async () => {
    const calls = await submit("anna.muster@example.org", json(202));
    expect(await screen.findByText(SAME_MESSAGE)).toBeInTheDocument();
    expect(calls).toEqual([
      {
        method: "POST",
        path: "/api/v1/sign-in/requests",
        body: JSON.stringify({ email: "anna.muster@example.org" }),
      },
    ]);
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("shows the message of the rate limit for a 429", async () => {
    await submit("anna.muster@example.org", problem(429, "rate-limited"));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Zu viele Anfragen. Versuchen Sie es in einigen Minuten erneut.",
    );
    expect(screen.queryByText(SAME_MESSAGE)).not.toBeInTheDocument();
  });

  it("keeps the address after an error", async () => {
    await submit("anna.muster@example.org", problem(503, "unavailable"));
    expect(await screen.findByRole("alert")).toBeInTheDocument();
    expect(screen.getByLabelText(/E-Mail-Adresse/)).toHaveValue("anna.muster@example.org");
  });
});
