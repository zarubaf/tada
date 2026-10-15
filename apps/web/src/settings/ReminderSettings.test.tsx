import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { createApi, type ReminderSettings as Settings } from "../api/client";
import { Router } from "../router/Router";
import { ReminderSettings } from "./ReminderSettings";
import { timeZonesWith } from "./timeZones";

function json(status: number, body: unknown) {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": status < 400 ? "application/json" : "application/problem+json" },
  });
}

const linked: Settings = { opted_in_at: null, time_zone: "Europe/Zurich", telegram_linked: true };

/** A fake server with the settings `initial`; `answer` answers each change. */
function setup(
  initial: Settings,
  answer?: (body: { opt_in: boolean; time_zone: string }) => Response,
) {
  const posts: { opt_in: boolean; time_zone: string }[] = [];
  const fetch = async (request: Request) => {
    const key = `${request.method} ${new URL(request.url).pathname}`;
    if (key === "GET /api/v1/account/reminders") {
      return json(200, initial);
    }
    if (key === "POST /api/v1/account/reminders/set" && answer) {
      const body = (await request.json()) as { opt_in: boolean; time_zone: string };
      posts.push(body);
      return answer(body);
    }
    throw new Error(`unexpected ${key}`);
  };
  const onStatus = vi.fn();
  const onFailure = vi.fn();
  render(
    <Router>
      <ReminderSettings
        api={createApi(fetch as unknown as typeof globalThis.fetch)}
        onStatus={onStatus}
        onFailure={onFailure}
      />
    </Router>,
  );
  return { posts, onStatus, onFailure };
}

const user = userEvent.setup({ delay: null });

describe("ReminderSettings", () => {
  it("opts in with the saved time zone when Telegram is linked", async () => {
    const { posts, onStatus } = setup(linked, (body) =>
      json(200, { ...linked, opted_in_at: "2030-05-18T08:00:00Z", time_zone: body.time_zone }),
    );

    const toggle = await screen.findByRole("switch", { name: "Erinnerungen in Telegram erhalten" });
    expect(toggle).not.toBeChecked();
    await user.click(toggle);

    await waitFor(() => expect(toggle).toBeChecked());
    expect(posts).toEqual([{ opt_in: true, time_zone: "Europe/Zurich" }]);
    expect(onStatus).toHaveBeenLastCalledWith("Erinnerungen sind eingeschaltet.");
  });

  it("disables the opt-in without a Telegram link and shows the way to the link", async () => {
    setup({ ...linked, telegram_linked: false });

    const toggle = await screen.findByRole("switch", { name: "Erinnerungen in Telegram erhalten" });
    expect(toggle).toBeDisabled();
    expect(toggle).toHaveAccessibleDescription(
      "Für Erinnerungen braucht tada ein verknüpftes Telegram-Konto. Telegram verknüpfen",
    );
    expect(screen.getByRole("link", { name: "Telegram verknüpfen" })).toHaveAttribute(
      "href",
      "/settings?tab=telegram",
    );
  });

  it("lets a member opt out after the link ended", async () => {
    const optedIn = { ...linked, telegram_linked: false, opted_in_at: "2030-05-18T08:00:00Z" };
    const { posts } = setup(optedIn, () => json(200, { ...optedIn, opted_in_at: null }));

    const toggle = await screen.findByRole("switch", { name: "Erinnerungen in Telegram erhalten" });
    expect(toggle).toBeEnabled();
    await user.click(toggle);

    await waitFor(() => expect(toggle).toBeDisabled());
    expect(posts).toEqual([{ opt_in: false, time_zone: "Europe/Zurich" }]);
    // The disabled switch drops its focus: the focus goes to the hint about the link.
    await waitFor(() => expect(screen.getByText(/Für Erinnerungen braucht tada/)).toHaveFocus());
  });

  it("says why a member who is in without a link cannot change the time zone", async () => {
    const optedIn = { ...linked, telegram_linked: false, opted_in_at: "2030-05-18T08:00:00Z" };
    const { posts, onFailure } = setup(optedIn, () =>
      json(422, {
        code: "validation-failed",
        status: 422,
        request_id: "r1",
        errors: [{ pointer: "/opt_in", code: "telegram-not-linked" }],
      }),
    );

    expect(
      await screen.findByText(/Ihr Telegram-Konto ist nicht mehr verknüpft\. tada schickt/),
    ).toBeInTheDocument();
    const input = screen.getByRole("combobox", { name: "Zeitzone" });
    await user.clear(input);
    await user.type(input, "Berlin");
    await user.click(await screen.findByRole("option", { name: "Europe/Berlin" }));

    expect(await screen.findByText(/Dann können Sie die Zeitzone ändern\./)).toBeInTheDocument();
    expect(posts).toEqual([{ opt_in: true, time_zone: "Europe/Berlin" }]);
    expect(onFailure).not.toHaveBeenCalledWith(
      expect.stringContaining("schalten Sie die Erinnerungen ein"),
    );
  });

  it("loads the settings again when the server says that the link is gone", async () => {
    let presses = 0;
    const { onFailure } = setup(linked, () => {
      presses += 1;
      return json(422, {
        code: "validation-failed",
        status: 422,
        request_id: "r1",
        errors: [{ pointer: "/opt_in", code: "telegram-not-linked" }],
      });
    });

    await user.click(await screen.findByRole("switch"));

    await waitFor(() =>
      expect(onFailure).toHaveBeenLastCalledWith(
        "Ihr Telegram-Konto ist nicht mehr verknüpft. Verknüpfen Sie es zuerst, dann schalten Sie die Erinnerungen ein.",
      ),
    );
    expect(presses).toBe(1);
  });

  it("saves a time zone that the member picks from the list", async () => {
    const { posts, onStatus } = setup(linked, (body) =>
      json(200, { ...linked, time_zone: body.time_zone }),
    );

    const input = await screen.findByRole("combobox", { name: "Zeitzone" });
    expect(input).toHaveValue("Europe/Zurich");
    await user.clear(input);
    await user.type(input, "new york");
    await user.click(await screen.findByRole("option", { name: "America/New York" }));

    await waitFor(() =>
      expect(onStatus).toHaveBeenLastCalledWith("Zeitzone America/New York gespeichert."),
    );
    expect(posts).toEqual([{ opt_in: false, time_zone: "America/New_York" }]);
    expect(input).toHaveValue("America/New York");
  });

  it("shows the field error of an unknown time zone", async () => {
    setup(linked, () =>
      json(422, {
        code: "validation-failed",
        status: 422,
        request_id: "r1",
        errors: [{ pointer: "/time_zone", code: "invalid-time-zone" }],
      }),
    );

    const input = await screen.findByRole("combobox", { name: "Zeitzone" });
    await user.clear(input);
    await user.type(input, "Berlin");
    await user.click(await screen.findByRole("option", { name: "Europe/Berlin" }));

    expect(
      await screen.findByText(
        "tada kennt diese Zeitzone nicht. Wählen Sie eine Zeitzone aus der Liste.",
      ),
    ).toBeInTheDocument();
  });
});

describe("timeZonesWith", () => {
  it("keeps a zone of the server that the browser does not list", () => {
    expect(timeZonesWith("Etc/Invented")).toContain("Etc/Invented");
    expect(timeZonesWith("Europe/Zurich").filter((zone) => zone === "Europe/Zurich")).toHaveLength(
      1,
    );
  });
});
