import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";
import { BERND, EVENT, json, ME, problem, renderWork } from "../test/fakeWorkServer";
import { CommitmentsPage } from "./CommitmentsPage";

const user = userEvent.setup({ delay: null });

const generators = { id: "i1", local_id: "INS-001", name: "Testwil Generatoren AG", version: 1 };

function commitment(over: Record<string, unknown> = {}) {
  return {
    id: "c1",
    local_id: "COM-001",
    event_id: EVENT.id,
    text: "Generator delivery Friday 15:00",
    condition: "subject to signed order",
    promisor: { kind: "institution", id: "i1", local_id: "INS-001", name: generators.name },
    owner_user_id: ME,
    workstream_id: "w1",
    due_date: "2030-05-17",
    status: "conditional",
    version: 3,
    evidence: [
      {
        record_version: 1,
        proposal_id: "p1",
        source_version_id: "s1",
        captured_at: "2030-05-01T10:30:00Z",
        start_offset: 0,
        end_offset: 10,
        quote: "Lieferung am Freitag, wenn bestellt.",
        page: 2,
      },
    ],
    ...over,
  };
}

const render = (setup: Parameters<typeof renderWork>[1] = {}) =>
  renderWork((api) => <CommitmentsPage api={api} />, setup);

afterEach(() => window.history.replaceState(null, "", "/"));

describe("CommitmentsPage", () => {
  it("the commitment register shows a conditional commitment with its condition", async () => {
    render({ lists: { "/commitments": [commitment()] } });

    const table = await screen.findByRole("table", { name: "Zusagen" });
    expect(within(table).getByText("COM-001")).toBeInTheDocument();
    expect(within(table).getByText("Generator delivery Friday 15:00")).toBeInTheDocument();
    expect(within(table).getByText("Bedingung: subject to signed order")).toBeInTheDocument();
    expect(within(table).getByText("Testwil Generatoren AG")).toBeInTheDocument();
    expect(within(table).getByText("Anna Muster")).toBeInTheDocument();
    const status = within(table).getByText("bedingt");
    expect(status.closest("[data-tone]")).toHaveAttribute("data-tone", "warning");
  });

  it("shows the evidence with the capture time of the source version", async () => {
    render({ lists: { "/commitments": [commitment()] } });
    await screen.findByRole("table", { name: "Zusagen" });

    await user.click(screen.getByRole("button", { name: "Beleg zu COM-001" }));

    const sheet = await screen.findByRole("dialog", { name: "Beleg zu COM-001" });
    expect(within(sheet).getByText("Lieferung am Freitag, wenn bestellt.")).toBeInTheDocument();
    expect(within(sheet).getByText("erfasst am 01.05.2030, 12:30")).toBeInTheDocument();
    expect(within(sheet).getByText("Seite 2")).toBeInTheDocument();
  });

  it("offers no evidence control without evidence", async () => {
    render({ lists: { "/commitments": [commitment({ evidence: [] })] } });
    await screen.findByRole("table", { name: "Zusagen" });
    expect(screen.queryByRole("button", { name: "Beleg zu COM-001" })).not.toBeInTheDocument();
  });

  it("make firm needs a reason", async () => {
    const { calls } = render({
      answers: {
        "POST /firm": () =>
          json(
            200,
            commitment({ status: "firm", firm_reason: "Bestellung unterzeichnet", version: 4 }),
          ),
      },
      lists: { "/commitments": [commitment()] },
    });
    await screen.findByRole("table", { name: "Zusagen" });

    await user.click(screen.getByRole("button", { name: "COM-001 verbindlich machen" }));
    const dialog = await screen.findByRole("dialog", { name: "COM-001 verbindlich machen" });
    await user.click(within(dialog).getByRole("button", { name: "Verbindlich machen" }));

    expect(
      within(dialog).getByText("Geben Sie einen Grund an. Er hat 1 bis 500 Zeichen."),
    ).toBeInTheDocument();
    expect(calls.some((c) => c.call.endsWith("/firm"))).toBe(false);

    await user.type(
      within(dialog).getByRole("textbox", { name: "Grund (Pflichtfeld)" }),
      "Bestellung unterzeichnet",
    );
    await user.click(within(dialog).getByRole("button", { name: "Verbindlich machen" }));

    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(calls.find((c) => c.call.endsWith("/firm"))?.body).toEqual({
      reason: "Bestellung unterzeichnet",
      expected_version: 3,
    });
    const table = screen.getByRole("table", { name: "Zusagen" });
    expect(within(table).getByText("verbindlich")).toBeInTheDocument();
    expect(within(table).getByText("Bedingung: subject to signed order")).toBeInTheDocument();
    expect(
      within(table).getByText("Verbindlich, weil: Bestellung unterzeichnet"),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "COM-001 verbindlich machen" }),
    ).not.toBeInTheDocument();
  });

  it("offers make firm only for a conditional commitment", async () => {
    render({ lists: { "/commitments": [commitment({ status: "firm" })] } });
    await screen.findByRole("table", { name: "Zusagen" });
    expect(
      screen.queryByRole("button", { name: "COM-001 verbindlich machen" }),
    ).not.toBeInTheDocument();
  });

  it("hides the change controls from a member who is neither owner nor lead nor manager", async () => {
    render({ lists: { "/commitments": [commitment({ owner_user_id: BERND })] } });
    await screen.findByRole("table", { name: "Zusagen" });
    expect(screen.queryByRole("button", { name: "COM-001 bearbeiten" })).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "COM-001 verbindlich machen" }),
    ).not.toBeInTheDocument();
  });

  it("a version conflict of make firm shows the reload message", async () => {
    render({
      answers: { "POST /firm": () => problem(409, "record-version-conflict") },
      lists: { "/commitments": [commitment()] },
    });
    await screen.findByRole("table", { name: "Zusagen" });

    await user.click(screen.getByRole("button", { name: "COM-001 verbindlich machen" }));
    const dialog = await screen.findByRole("dialog");
    await user.type(within(dialog).getByRole("textbox", { name: "Grund (Pflichtfeld)" }), "ok");
    await user.click(within(dialog).getByRole("button", { name: "Verbindlich machen" }));

    expect(
      await screen.findByText("Der Eintrag wurde inzwischen geändert. Die Liste ist neu geladen."),
    ).toBeInTheDocument();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("the commitment form offers no firm status and shows the fixed condition", async () => {
    render({ lists: { "/commitments": [commitment()] } });
    await screen.findByRole("table", { name: "Zusagen" });

    await user.click(screen.getByRole("button", { name: "COM-001 bearbeiten" }));
    expect(screen.queryByRole("textbox", { name: "Bedingung" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /Status/ }));

    const options = within(await screen.findByRole("listbox")).getAllByRole("option");
    expect(options.map((option) => option.textContent)).toEqual([
      "bedingt",
      "erfüllt",
      "gebrochen",
      "zurückgezogen",
    ]);
  });

  it("creates a commitment of a promisor with a condition", async () => {
    const { calls } = render({
      answers: {
        "POST /commitments": () => json(201, commitment({ id: "c2", local_id: "COM-002" })),
      },
      lists: { "/commitments": [], "/persons": [], "/institutions": [generators] },
    });
    await screen.findByText("Noch keine Zusagen");

    await user.type(screen.getByRole("textbox", { name: "Zusage (Pflichtfeld)" }), "Lieferung");
    await user.click(await screen.findByRole("button", { name: /Zugesagt von/ }));
    await user.click(
      await screen.findByRole("option", { name: "Testwil Generatoren AG (INS-001)" }),
    );
    await user.type(screen.getByRole("textbox", { name: "Bedingung" }), "bei Bestellung");
    await user.click(screen.getByRole("button", { name: "Erfassen" }));

    expect(await screen.findByText("COM-002")).toBeInTheDocument();
    expect(calls.find((c) => c.call.startsWith("POST /api/v1/events"))?.body).toEqual({
      text: "Lieferung",
      owner_user_id: ME,
      promisor: { kind: "institution", id: "i1" },
      condition: "bei Bestellung",
    });
  });

  it("asks for a promisor before it sends", async () => {
    const { calls } = render({
      lists: { "/commitments": [], "/persons": [], "/institutions": [] },
    });
    await screen.findByText("Noch keine Zusagen");

    await user.type(screen.getByRole("textbox", { name: "Zusage (Pflichtfeld)" }), "Lieferung");
    await user.click(screen.getByRole("button", { name: "Erfassen" }));

    expect(
      await screen.findByText("Wählen Sie eine Person oder eine Institution."),
    ).toBeInTheDocument();
    expect(calls.some((c) => c.call.startsWith("POST"))).toBe(false);
  });
});
