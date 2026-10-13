import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";
import { BERND, EVENT, json, ME, pagedList, problem, renderWork } from "../test/fakeWorkServer";
import { MilestonesPage } from "./MilestonesPage";

const user = userEvent.setup({ delay: null });

function milestone(over: Record<string, unknown> = {}) {
  return {
    id: "m1",
    local_id: "MIL-001",
    event_id: EVENT.id,
    title: "Bewilligung liegt vor",
    redacted: false,
    date: "2030-05-01",
    owner_user_id: ME,
    workstream_id: "w1",
    status: "open",
    warnings: [],
    version: 2,
    evidence: [],
    can_change: true,
    next_statuses: ["reached", "canceled"],
    ...over,
  };
}

function action(over: Record<string, unknown> = {}) {
  return {
    id: "a1",
    local_id: "ACT-001",
    event_id: EVENT.id,
    title: "Gesuch einreichen",
    redacted: false,
    owner_user_id: ME,
    milestone_id: "m1",
    due_date: "2030-05-10",
    status: "open",
    version: 1,
    evidence: [],
    can_change: true,
    next_statuses: [],
    ...over,
  };
}

const actions = [
  action(),
  action({ id: "a2", local_id: "ACT-002", status: "in-progress" }),
  action({ id: "a3", local_id: "ACT-003", status: "done" }),
  action({ id: "a4", local_id: "ACT-004", milestone_id: null }),
];

const render = (setup: Parameters<typeof renderWork>[1] = {}) =>
  renderWork((api) => <MilestonesPage api={api} />, {
    ...setup,
    lists: { "/actions": actions, ...setup.lists },
  });

afterEach(() => window.history.replaceState(null, "", "/"));

describe("MilestonesPage", () => {
  it("lists milestones with date, owner, workstream, open actions and status", async () => {
    render({ lists: { "/milestones": [milestone()] } });

    const table = await screen.findByRole("table", { name: "Meilensteine" });
    const row = within(table).getByText("MIL-001").closest("tr") as HTMLElement;
    expect(within(row).getByText("Bewilligung liegt vor")).toBeInTheDocument();
    expect(within(row).getByText("01.05.2030")).toBeInTheDocument();
    expect(within(row).getByText("Anna Muster")).toBeInTheDocument();
    expect(within(row).getByText("Bodenbetrieb")).toBeInTheDocument();
    // ACT-001 is open and ACT-002 in progress; ACT-003 is done and ACT-004 has no milestone.
    expect(within(row).getByText("2")).toBeInTheDocument();
    expect(within(row).getByText("offen")).toBeInTheDocument();
  });

  it("opens the milestone named by the hash of the address and focuses it", async () => {
    render({
      hash: "#MIL-002",
      lists: {
        "/milestones": [
          milestone(),
          milestone({ id: "m2", local_id: "MIL-002", title: "Festzelt steht" }),
        ],
      },
    });

    const row = (await screen.findByText("Festzelt steht")).closest("tr");
    expect(row).toHaveAttribute("id", "MIL-002");
    await waitFor(() => expect(row).toHaveFocus());
    expect(row).toHaveAttribute("data-linked");
  });

  it("loads further pages until the milestone named by the hash appears", async () => {
    render({
      hash: "#MIL-002",
      answers: {
        "GET /milestones": pagedList([
          [milestone()],
          [milestone({ id: "m2", local_id: "MIL-002", title: "Festzelt steht" })],
        ]),
      },
    });

    const row = (await screen.findByText("Festzelt steht")).closest("tr");
    await waitFor(() => expect(row).toHaveFocus());
  });

  it("shows both warnings of a milestone", async () => {
    render({
      lists: {
        "/milestones": [
          milestone({
            warnings: [{ kind: "overdue" }, { kind: "late-action", action_id: "a2" }],
          }),
        ],
      },
    });

    const table = await screen.findByRole("table", { name: "Meilensteine" });
    expect(within(table).getByText("Datum überschritten")).toBeInTheDocument();
    expect(within(table).getByText("ACT-002 ist erst danach fällig")).toBeInTheDocument();
  });

  it("creates a milestone with title, date and the member as owner", async () => {
    const created = milestone({ id: "m2", local_id: "MIL-002", title: "Festzelt steht" });
    const { calls } = render({
      answers: { "POST /milestones": () => json(201, created) },
      lists: { "/milestones": [] },
    });
    await screen.findByText("Noch keine Meilensteine");

    await user.type(screen.getByRole("textbox", { name: "Titel (Pflichtfeld)" }), "Festzelt steht");
    const date = screen.getByLabelText("Datum (Pflichtfeld)");
    await user.type(date, "2030-05-20");
    await user.click(screen.getByRole("button", { name: "Erfassen" }));

    expect(await screen.findByText("MIL-002")).toBeInTheDocument();
    expect(calls.find((c) => c.call.startsWith("POST"))?.body).toEqual({
      title: "Festzelt steht",
      date: "2030-05-20",
      owner_user_id: ME,
    });
  });

  it("asks for the date before it creates a milestone", async () => {
    const { calls } = render({ lists: { "/milestones": [] } });
    await screen.findByText("Noch keine Meilensteine");

    await user.type(screen.getByRole("textbox", { name: "Titel (Pflichtfeld)" }), "Festzelt steht");
    await user.click(screen.getByRole("button", { name: "Erfassen" }));

    expect(await screen.findByText("Geben Sie ein gültiges Datum an.")).toBeInTheDocument();
    expect(calls.some((c) => c.call.startsWith("POST"))).toBe(false);
  });

  it("marks a milestone as reached", async () => {
    const { calls } = render({
      answers: {
        "PATCH /milestones/m1": () =>
          json(200, milestone({ status: "reached", version: 3, next_statuses: ["open"] })),
      },
      lists: { "/milestones": [milestone()] },
    });
    await screen.findByRole("table", { name: "Meilensteine" });

    await user.click(screen.getByRole("button", { name: "MIL-001 als erreicht markieren" }));

    expect(await screen.findByText("erreicht")).toBeInTheDocument();
    expect(calls.find((c) => c.call.startsWith("PATCH"))?.body).toEqual({
      status: "reached",
      expected_version: 2,
    });
    expect(screen.getByRole("button", { name: "MIL-001 wieder öffnen" })).toBeInTheDocument();
  });

  it("opens a reached milestone again", async () => {
    const { calls } = render({
      answers: { "PATCH /milestones/m1": () => json(200, milestone({ version: 4 })) },
      lists: {
        "/milestones": [milestone({ status: "reached", version: 3, next_statuses: ["open"] })],
      },
    });
    await screen.findByRole("table", { name: "Meilensteine" });

    await user.click(screen.getByRole("button", { name: "MIL-001 wieder öffnen" }));

    expect(await screen.findByText("offen")).toBeInTheDocument();
    expect(calls.find((c) => c.call.startsWith("PATCH"))?.body).toEqual({
      status: "open",
      expected_version: 3,
    });
  });

  it("a version conflict of a status change loads the rows again", async () => {
    const { calls } = render({
      answers: { "PATCH /milestones/m1": () => problem(409, "record-version-conflict") },
      lists: { "/milestones": [milestone()] },
    });
    await screen.findByRole("table", { name: "Meilensteine" });

    await user.click(screen.getByRole("button", { name: "MIL-001 als erreicht markieren" }));

    expect(
      await screen.findByText("Der Eintrag wurde inzwischen geändert. Die Liste ist neu geladen."),
    ).toBeInTheDocument();
    await waitFor(() =>
      expect(
        calls.filter((c) => c.call === `GET /api/v1/events/${EVENT.id}/milestones`),
      ).toHaveLength(2),
    );
  });

  it("sends only the changed fields of an edit", async () => {
    const { calls } = render({
      answers: {
        "PATCH /milestones/m1": () => json(200, milestone({ owner_user_id: BERND, version: 3 })),
      },
      lists: { "/milestones": [milestone()] },
    });
    await screen.findByRole("table", { name: "Meilensteine" });

    await user.click(screen.getByRole("button", { name: "MIL-001 bearbeiten" }));
    await user.click(screen.getByRole("button", { name: /Verantwortlich/ }));
    await user.click(await screen.findByRole("option", { name: "Bernd Beispiel" }));
    await user.click(screen.getByRole("button", { name: "Speichern" }));

    await waitFor(() =>
      expect(calls.find((c) => c.call.startsWith("PATCH"))?.body).toEqual({
        owner_user_id: BERND,
        expected_version: 2,
      }),
    );
  });

  it("cancels a milestone in the form, which offers only the allowed statuses", async () => {
    const { calls } = render({
      answers: {
        "PATCH /milestones/m1": () =>
          json(200, milestone({ status: "canceled", version: 3, next_statuses: ["open"] })),
      },
      lists: { "/milestones": [milestone()] },
    });
    await screen.findByRole("table", { name: "Meilensteine" });

    await user.click(screen.getByRole("button", { name: "MIL-001 bearbeiten" }));
    await user.click(screen.getByRole("button", { name: /Status/ }));
    const options = within(await screen.findByRole("listbox")).getAllByRole("option");
    expect(options.map((option) => option.textContent)).toEqual(["offen", "erreicht", "abgesagt"]);
    await user.click(screen.getByRole("option", { name: "abgesagt" }));
    await user.click(screen.getByRole("button", { name: "Speichern" }));

    expect(await screen.findByText("abgesagt", { selector: "td *" })).toBeInTheDocument();
    expect(calls.find((c) => c.call.startsWith("PATCH"))?.body).toEqual({
      status: "canceled",
      expected_version: 2,
    });
  });

  it("offers no change of a milestone that the caller cannot change", async () => {
    render({
      lists: { "/milestones": [milestone({ can_change: false, next_statuses: [] })] },
    });
    await screen.findByRole("table", { name: "Meilensteine" });

    expect(screen.queryByRole("button", { name: "MIL-001 bearbeiten" })).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "MIL-001 als erreicht markieren" }),
    ).not.toBeInTheDocument();
  });

  it("shows the evidence of a milestone in a sheet", async () => {
    render({
      lists: {
        "/milestones": [
          milestone({
            evidence: [
              {
                record_version: 1,
                proposal_id: "p1",
                source_version_id: "s1",
                captured_at: "2030-03-01T10:30:00Z",
                start_offset: 0,
                end_offset: 20,
                quote: "Die Bewilligung kommt bis Ende April.",
              },
            ],
          }),
        ],
      },
    });
    await screen.findByRole("table", { name: "Meilensteine" });

    await user.click(screen.getByRole("button", { name: "Beleg zu MIL-001" }));

    const sheet = await screen.findByRole("dialog", { name: "Beleg zu MIL-001" });
    expect(within(sheet).getByText(/Die Bewilligung kommt bis Ende April/)).toBeInTheDocument();
    expect(within(sheet).getByText("Stützt die Version 1 des Meilensteins")).toBeInTheDocument();
  });
});
