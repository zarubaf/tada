import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { createApi, type Problem } from "../api/client";
import { Router } from "../router/Router";
import { SLOW } from "../test/timeouts";
import { CreateEventForm } from "./CreateEventForm";

function json(status: number, body: unknown, contentType = "application/json") {
  return new Response(JSON.stringify(body), { status, headers: { "Content-Type": contentType } });
}

function problem(status: number, code: string, errors?: Problem["errors"]) {
  const body = { type: "", code, title: "", status, instance: "", request_id: "r1", errors };
  return json(status, body, "application/problem+json");
}

/** A fake server: each call returns the next response and records the JSON body. */
function fakeApi(...responses: Response[]) {
  const bodies: Record<string, unknown>[] = [];
  const fetch = vi.fn(async (request: Request) => {
    bodies.push(await request.clone().json());
    const response = responses.shift();
    if (!response) {
      throw new Error("no more responses");
    }
    return response;
  });
  return { api: createApi(fetch as unknown as typeof globalThis.fetch), bodies };
}

const user = userEvent.setup({ delay: null });

function renderForm(api: ReturnType<typeof createApi>) {
  window.history.replaceState(null, "", "/events/new");
  return render(
    <Router>
      <CreateEventForm api={api} />
    </Router>,
  );
}

afterEach(() => window.history.replaceState(null, "", "/"));

const created = {
  id: "0199b8e0-0000-7000-8000-000000000009",
  key: "TEST30",
  name: "Tag der offenen Tür Testwil",
  time_zone: "Europe/Zurich",
  version: 1,
  created_at: "2030-05-18T08:00:00Z",
};

describe("CreateEventForm", () => {
  it(
    "defaults the time zone to Europe/Zurich",
    () => {
      renderForm(fakeApi().api);
      expect(screen.getByLabelText("Zeitzone")).toHaveValue("Europe/Zurich");
    },
    SLOW,
  );

  it(
    "creates the event with a client UUIDv7 and opens its page",
    async () => {
      const { api, bodies } = fakeApi(json(201, created));
      renderForm(api);

      await user.type(screen.getByLabelText("Kürzel"), "test30");
      await user.type(screen.getByLabelText("Name"), "Tag der offenen Tür Testwil");
      await user.click(screen.getByRole("button", { name: "Anlass erfassen" }));

      await vi.waitFor(() => expect(window.location.pathname).toBe(`/events/${created.id}`));
      expect(bodies[0]).toMatchObject({
        key: "TEST30",
        name: "Tag der offenen Tür Testwil",
        time_zone: "Europe/Zurich",
      });
      expect(bodies[0]?.id).toMatch(/^[0-9a-f-]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-/);
    },
    SLOW,
  );

  it(
    "shows a taken key at the field and retries with the same ID",
    async () => {
      const { api, bodies } = fakeApi(
        problem(422, "validation-failed", [{ pointer: "/key", code: "taken" }]),
        json(201, created),
      );
      renderForm(api);

      await user.type(screen.getByLabelText("Kürzel"), "TEST30");
      await user.type(screen.getByLabelText("Name"), "Testanlass");
      await user.click(screen.getByRole("button", { name: "Anlass erfassen" }));

      expect(await screen.findByText("Dieses Kürzel ist schon vergeben.")).toBeInTheDocument();
      expect(screen.getByLabelText("Kürzel")).toBeInvalid();

      await user.click(screen.getByRole("button", { name: "Anlass erfassen" }));
      await vi.waitFor(() => expect(window.location.pathname).toBe(`/events/${created.id}`));
      expect(bodies[1]?.id).toBe(bodies[0]?.id);
    },
    SLOW,
  );

  it(
    "shows the message of another problem code",
    async () => {
      const { api } = fakeApi(problem(403, "forbidden"));
      renderForm(api);

      await user.type(screen.getByLabelText("Kürzel"), "TEST30");
      await user.type(screen.getByLabelText("Name"), "Testanlass");
      await user.click(screen.getByRole("button", { name: "Anlass erfassen" }));

      expect(await screen.findByRole("alert")).toHaveTextContent(
        "Sie haben keine Berechtigung für diese Aktion.",
      );
    },
    SLOW,
  );

  it(
    "does not send a key that is too short",
    async () => {
      const { api, bodies } = fakeApi();
      renderForm(api);

      await user.type(screen.getByLabelText("Kürzel"), "A");
      await user.type(screen.getByLabelText("Name"), "Testanlass");
      await user.click(screen.getByRole("button", { name: "Anlass erfassen" }));

      expect(
        await screen.findByText("Das Kürzel hat 2 bis 8 Grossbuchstaben oder Ziffern."),
      ).toBeInTheDocument();
      expect(bodies).toHaveLength(0);
      expect(screen.getByLabelText("Kürzel")).toHaveFocus();
    },
    SLOW,
  );

  it(
    "moves focus to the first invalid field after the server refuses a value",
    async () => {
      const { api } = fakeApi(
        problem(422, "validation-failed", [{ pointer: "/key", code: "taken" }]),
      );
      renderForm(api);

      await user.type(screen.getByLabelText("Kürzel"), "TEST30");
      await user.type(screen.getByLabelText("Name"), "Testanlass");
      await user.click(screen.getByRole("button", { name: "Anlass erfassen" }));

      await vi.waitFor(() => expect(screen.getByLabelText("Kürzel")).toHaveFocus());
    },
    SLOW,
  );

  it(
    "announces a failure in a live region that was there before",
    async () => {
      const { api } = fakeApi(problem(403, "forbidden"));
      renderForm(api);

      const region = screen.getByRole("alert");
      expect(region).toBeEmpty();
      await user.type(screen.getByLabelText("Kürzel"), "TEST30");
      await user.type(screen.getByLabelText("Name"), "Testanlass");
      await user.click(screen.getByRole("button", { name: "Anlass erfassen" }));

      await vi.waitFor(() =>
        expect(region).toHaveTextContent("Sie haben keine Berechtigung für diese Aktion."),
      );
    },
    SLOW,
  );
});
