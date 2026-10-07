import { render, screen, waitFor } from "@testing-library/react";
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

  it("shows the same message for an unknown address and for a known one", async () => {
    await submit("unbekannt@example.org", json(202));
    expect(await screen.findByRole("status")).toHaveTextContent(SAME_MESSAGE);
  });

  it("has the live region before the message appears", async () => {
    const { api } = fakeApi(json(202));
    render(
      <Router>
        <SignInPage api={api} />
      </Router>,
    );
    const region = screen.getByRole("status");
    expect(region).toBeEmptyDOMElement();
    await userEvent.click(screen.getByLabelText(/E-Mail-Adresse/));
    await userEvent.paste("anna.muster@example.org");
    await userEvent.click(screen.getByRole("button", { name: "Anmeldelink senden" }));
    await waitFor(() => expect(region).toHaveTextContent(SAME_MESSAGE));
    expect(screen.getByRole("status")).toBe(region);
  });

  it("names the wait of Retry-After for a 429, moves focus and enables the button after it", async () => {
    await submit("anna.muster@example.org", problem(429, "rate-limited", { "Retry-After": "1" }));
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("Zu viele Anfragen. Versuchen Sie es in 1 Sekunde erneut.");
    expect(alert).toHaveFocus();
    const button = screen.getByRole("button", { name: "Anmeldelink senden" });
    expect(button).toBeDisabled();
    await waitFor(() => expect(button).toBeEnabled());
    expect(screen.queryByText(SAME_MESSAGE)).not.toBeInTheDocument();
  });

  it("shows the general message of the rate limit without Retry-After", async () => {
    await submit("anna.muster@example.org", problem(429, "rate-limited"));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Zu viele Anfragen. Versuchen Sie es in einigen Minuten erneut.",
    );
  });

  it("keeps the address after an error", async () => {
    await submit("anna.muster@example.org", problem(503, "unavailable"));
    expect(await screen.findByRole("alert")).toBeInTheDocument();
    expect(screen.getByLabelText(/E-Mail-Adresse/)).toHaveValue("anna.muster@example.org");
  });
});
