import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";
import { createApi } from "../api/client";
import { Route, Router, Routes } from "../router/Router";
import { SessionProvider } from "../session/SessionProvider";
import { TelegramPage } from "./TelegramPage";

const request = {
  id: "r1",
  telegram_user_id: 4711,
  telegram_name: "Bernd Beispiel",
  claimed_at: "2030-05-18T08:00:00Z",
};

function json(status: number, body: unknown, contentType = "application/json") {
  return new Response(JSON.stringify(body), { status, headers: { "Content-Type": contentType } });
}

function problem(status: number, code: string) {
  const body = { type: "", code, title: "", status, instance: "", request_id: "r1" };
  return json(status, body, "application/problem+json");
}

interface Setup {
  requests?: unknown[][];
  answers?: Record<string, () => Response>;
}

/** A fake server. `requests` has one list per `GET`; the last one repeats. */
function setup({ requests = [[request]], answers = {} }: Setup = {}) {
  const calls: string[] = [];
  let lists = 0;
  const own = { organization_id: "o1", name: "Fliegergruppe Testwil", role: "member" };
  const session = {
    user_id: "u1",
    display_name: "Anna Muster",
    organization: own,
    memberships: [own],
  };
  const fetch = async (call: Request) => {
    const { pathname } = new URL(call.url);
    const key = `${call.method} ${pathname}`;
    calls.push(key);
    const answer = answers[key];
    if (answer) {
      return answer();
    }
    if (pathname.endsWith("/session")) {
      return json(200, session);
    }
    if (key === "GET /api/v1/telegram/link-requests") {
      const items = requests[Math.min(lists, requests.length - 1)];
      lists += 1;
      return json(200, { items });
    }
    if (key === "POST /api/v1/telegram/link-codes") {
      return json(201, { code: "K7M3-QX92", expires_at: "2030-05-18T08:10:00Z" });
    }
    if (key === "POST /api/v1/telegram/link-requests/r1/confirm") {
      return json(200, { telegram_user_id: 4711 });
    }
    throw new Error(`unexpected ${key}`);
  };
  const api = createApi(fetch as unknown as typeof globalThis.fetch);
  window.history.replaceState(null, "", "/settings/telegram");
  render(
    <Router>
      <SessionProvider api={api}>
        <Routes>
          <Route path="/settings/telegram">
            <TelegramPage api={api} />
          </Route>
        </Routes>
      </SessionProvider>
    </Router>,
  );
  return { calls };
}

const user = userEvent.setup({ delay: null });

afterEach(() => window.history.replaceState(null, "", "/"));

const CONFIRM = "POST /api/v1/telegram/link-requests/r1/confirm";

describe("TelegramPage", () => {
  it("shows no code before the member creates one", async () => {
    const { calls } = setup();

    await screen.findByRole("table", { name: "Offene Anfragen" });
    expect(screen.queryByText("K7M3-QX92")).not.toBeInTheDocument();
    expect(calls).not.toContain("POST /api/v1/telegram/link-codes");
  });

  it("shows the code with its expiry after the creation, and moves focus to it", async () => {
    setup();
    await screen.findByRole("table", { name: "Offene Anfragen" });

    await user.click(screen.getByRole("button", { name: "Code erstellen" }));

    const code = await screen.findByText("K7M3-QX92");
    expect(screen.getByText(/läuft ab um/)).toBeInTheDocument();
    await waitFor(() => expect(code.closest("[tabindex]")).toHaveFocus());
  });

  it("warns that the member must check the Telegram name", async () => {
    setup();

    expect(await screen.findByText(/Ihr eigenes Telegram-Konto/)).toBeInTheDocument();
  });

  it("shows the Telegram name and the time of an open request", async () => {
    setup();

    const table = await screen.findByRole("table", { name: "Offene Anfragen" });
    expect(within(table).getByText("Bernd Beispiel")).toBeInTheDocument();
    expect(table.querySelector("time")).toHaveAttribute("datetime", request.claimed_at);
  });

  it("asks before it links, and removes the request after the confirmation", async () => {
    const { calls } = setup();
    await user.click(await screen.findByRole("button", { name: "Bernd Beispiel bestätigen" }));

    const dialog = await screen.findByRole("alertdialog");
    expect(within(dialog).getByText(/Bernd Beispiel/)).toBeInTheDocument();
    expect(calls).not.toContain(CONFIRM);
    await user.click(within(dialog).getByRole("button", { name: "Verknüpfen" }));

    expect(await screen.findByText("Telegram-Konto verknüpft.")).toBeInTheDocument();
    expect(calls).toContain(CONFIRM);
    expect(screen.queryByRole("table", { name: "Offene Anfragen" })).not.toBeInTheDocument();
    await waitFor(() =>
      expect(screen.getByRole("heading", { name: "Offene Anfragen" })).toHaveFocus(),
    );
  });

  it("keeps the request when the member cancels", async () => {
    const { calls } = setup();
    await user.click(await screen.findByRole("button", { name: "Bernd Beispiel bestätigen" }));
    const dialog = await screen.findByRole("alertdialog");
    await user.click(within(dialog).getByRole("button", { name: "Abbrechen" }));

    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
    expect(screen.getByText("Bernd Beispiel")).toBeInTheDocument();
    expect(calls).not.toContain(CONFIRM);
  });

  it("keeps the request and says why when the confirmation fails", async () => {
    setup({ answers: { [CONFIRM]: () => problem(503, "unavailable") } });
    await user.click(await screen.findByRole("button", { name: "Bernd Beispiel bestätigen" }));
    const dialog = await screen.findByRole("alertdialog");
    await user.click(within(dialog).getByRole("button", { name: "Verknüpfen" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("nicht erreichbar");
    expect(screen.getByText("Bernd Beispiel")).toBeInTheDocument();
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Bernd Beispiel bestätigen" })).toHaveFocus(),
    );
  });

  it("asks for a new sign-in when the session is too old to link, and signs out on request", async () => {
    const { calls } = setup({
      answers: {
        [CONFIRM]: () => problem(403, "recent-sign-in-required"),
        "POST /api/v1/sign-out": () => new Response(null, { status: 204 }),
      },
    });
    await user.click(await screen.findByRole("button", { name: "Bernd Beispiel bestätigen" }));
    const dialog = await screen.findByRole("alertdialog");
    await user.click(within(dialog).getByRole("button", { name: "Verknüpfen" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Melden Sie sich für diesen Schritt neu an.",
    );
    await user.click(screen.getByRole("button", { name: "Neu anmelden" }));

    await waitFor(() => expect(window.location.pathname).toBe("/sign-in"));
    expect(calls).toContain("POST /api/v1/sign-out");
  });

  it("loads the requests again on request", async () => {
    setup({ requests: [[], [request]] });
    await screen.findByText("Keine offenen Anfragen");

    const refresh = screen.getByRole("button", { name: "Aktualisieren" });
    await user.click(refresh);

    expect(await screen.findByText("Bernd Beispiel")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Aktualisieren" })).toHaveFocus();
    expect(await screen.findByText("Anfragen aktualisiert.")).toBeInTheDocument();
  });

  it("shows the Telegram ID in the table and the ID and time in the dialog", async () => {
    setup();
    const table = await screen.findByRole("table", { name: "Offene Anfragen" });
    expect(within(table).getByText("4711")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Bernd Beispiel bestätigen" }));

    const dialog = await screen.findByRole("alertdialog");
    expect(dialog).toHaveTextContent("Telegram-ID 4711");
    expect(dialog).toHaveTextContent("2030");
  });

  it("opens the dialog with focus on the dialog, not on Verknüpfen", async () => {
    setup();
    await user.click(await screen.findByRole("button", { name: "Bernd Beispiel bestätigen" }));

    const dialog = await screen.findByRole("alertdialog");
    await waitFor(() => expect(dialog).toHaveFocus());
    expect(within(dialog).getByRole("button", { name: "Verknüpfen" })).not.toHaveFocus();
  });

  it("clears the code after a successful link", async () => {
    setup();
    await user.click(await screen.findByRole("button", { name: "Code erstellen" }));
    await screen.findByText("K7M3-QX92");
    await user.click(screen.getByRole("button", { name: "Bernd Beispiel bestätigen" }));
    const dialog = await screen.findByRole("alertdialog");
    await user.click(within(dialog).getByRole("button", { name: "Verknüpfen" }));

    await screen.findByText("Telegram-Konto verknüpft.");
    expect(screen.queryByText("K7M3-QX92")).not.toBeInTheDocument();
  });

  it("loads the list again when the request is gone", async () => {
    const { calls } = setup({
      requests: [[request], []],
      answers: { [CONFIRM]: () => problem(404, "not-found") },
    });
    await user.click(await screen.findByRole("button", { name: "Bernd Beispiel bestätigen" }));
    const dialog = await screen.findByRole("alertdialog");
    await user.click(within(dialog).getByRole("button", { name: "Verknüpfen" }));

    await screen.findByText("Keine offenen Anfragen");
    // The row with the focus is gone: focus goes to the heading of the list.
    await waitFor(() =>
      expect(screen.getByRole("heading", { name: "Offene Anfragen" })).toHaveFocus(),
    );
    expect(calls.filter((c) => c === "GET /api/v1/telegram/link-requests")).toHaveLength(2);
  });

  it("shows a failed list with a retry that takes focus", async () => {
    setup({
      answers: { "GET /api/v1/telegram/link-requests": () => problem(503, "unavailable") },
    });
    await screen.findByText(/nicht erreichbar/);

    await user.click(screen.getByRole("button", { name: "Erneut versuchen" }));

    await waitFor(() =>
      expect(screen.getByText(/nicht erreichbar/).closest("[role=alert]")).toHaveFocus(),
    );
  });

  it("shows a failed code creation in the alert", async () => {
    setup({
      answers: { "POST /api/v1/telegram/link-codes": () => problem(503, "unavailable") },
    });
    await screen.findByRole("table", { name: "Offene Anfragen" });

    await user.click(screen.getByRole("button", { name: "Code erstellen" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("nicht erreichbar");
    expect(screen.queryByText("K7M3-QX92")).not.toBeInTheDocument();
  });
});
