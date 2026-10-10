import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it } from "vitest";
import { type Api, createApi, type OrganizationRole } from "../api/client";
import { LiveRegion } from "../ui/LiveRegion";
import { RetentionSettings } from "./RetentionSettings";

function json(status: number, body: unknown, contentType = "application/json") {
  return new Response(JSON.stringify(body), { status, headers: { "Content-Type": contentType } });
}

function problem(status: number, code: string, errors?: unknown[]) {
  return json(
    status,
    { type: "", code, title: "", status, instance: "", request_id: "r1", errors },
    "application/problem+json",
  );
}

/** The page owns the live regions, as the organization page does. */
function Harness({ api, role }: { api: Api; role: OrganizationRole }) {
  const [status, setStatus] = useState<string>();
  const [failure, setFailure] = useState<string>();
  return (
    <>
      <LiveRegion kind="alert">{failure}</LiveRegion>
      <LiveRegion kind="status">{status}</LiveRegion>
      <RetentionSettings api={api} role={role} onStatus={setStatus} onFailure={setFailure} />
    </>
  );
}

function setup(role: OrganizationRole, ...responses: (() => Response)[]) {
  const calls: { call: string; body: unknown }[] = [];
  const fetch = async (request: Request) => {
    const text = request.method === "POST" ? await request.clone().text() : "";
    calls.push({
      call: `${request.method} ${new URL(request.url).pathname}`,
      body: text === "" ? undefined : JSON.parse(text),
    });
    const answer = responses.shift();
    if (!answer) {
      throw new Error("no more responses");
    }
    return answer();
  };
  const api = createApi(fetch as unknown as typeof globalThis.fetch);
  render(<Harness api={api} role={role} />);
  return calls;
}

const user = userEvent.setup({ delay: null });
const closedField = () => screen.findByRole("textbox", { name: "Abgeschlossene Anlässe (Monate)" });
const sourceField = () => screen.findByRole("textbox", { name: "Quellen ohne Anlass (Monate)" });
const off = { closed_event_months: null, source_months: null, version: 1 };

describe("RetentionSettings", () => {
  it("retention fields accept 1–120 and empty", async () => {
    const calls = setup(
      "owner",
      () => json(200, { closed_event_months: 24, source_months: 12, version: 3 }),
      () => json(200, { closed_event_months: 120, source_months: null, version: 4 }),
    );

    const closed = await closedField();
    expect(closed).toHaveValue("24");
    expect(await sourceField()).toHaveValue("12");

    await user.clear(closed);
    await user.type(closed, "120");
    await user.clear(await sourceField());
    await user.click(screen.getByRole("button", { name: "Speichern" }));

    expect(await screen.findByText("Fristen gespeichert.")).toBeInTheDocument();
    expect(calls[1]).toEqual({
      call: "POST /api/v1/organization/retention/set",
      body: { closed_event_months: 120, source_months: null, expected_version: 3 },
    });
  });

  it("shows empty fields as „keine automatische Löschung“", async () => {
    setup("owner", () => json(200, off));

    expect(await closedField()).toHaveValue("");
    expect(screen.getAllByText(/keine automatische Löschung/).length).toBeGreaterThan(0);
  });

  it("shows the accounting note", async () => {
    setup("owner", () => json(200, off));

    await closedField();
    expect(screen.getByText(/OR Art\. 958f/)).toHaveTextContent("10 Jahre");
  });

  it("shows the field error that the server names", async () => {
    setup(
      "owner",
      () => json(200, off),
      () =>
        problem(422, "validation-failed", [{ pointer: "/source_months", code: "out-of-range" }]),
    );

    await user.type(await sourceField(), "121");
    await user.click(screen.getByRole("button", { name: "Speichern" }));

    const field = await sourceField();
    await waitFor(() => expect(field).toBeInvalid());
    expect(screen.getByText("Geben Sie eine Zahl von 1 bis 120 ein.")).toBeVisible();
    expect(await closedField()).toBeValid();
  });

  it("does not send text that is no number", async () => {
    const calls = setup("owner", () => json(200, off));

    await user.type(await closedField(), "zwei");
    await user.click(screen.getByRole("button", { name: "Speichern" }));

    expect(await screen.findByText("Geben Sie eine Zahl von 1 bis 120 ein.")).toBeVisible();
    expect(calls).toHaveLength(1);
  });

  it("reloads the periods and the version after a conflict", async () => {
    const calls = setup(
      "owner",
      () => json(200, off),
      () => problem(409, "record-version-conflict"),
      () => json(200, { closed_event_months: 6, source_months: null, version: 2 }),
      () => json(200, { closed_event_months: 6, source_months: 12, version: 3 }),
    );

    await user.click(await screen.findByRole("button", { name: "Speichern" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("Jemand hat die Fristen");
    expect(await closedField()).toHaveValue("6");
    await user.type(await sourceField(), "12");
    await user.click(screen.getByRole("button", { name: "Speichern" }));
    await screen.findByText("Fristen gespeichert.");
    expect(calls[3]?.body).toEqual({
      closed_event_months: 6,
      source_months: 12,
      expected_version: 2,
    });
  });

  it("owner only: an admin reads the periods and cannot change them", async () => {
    setup("admin", () => json(200, { closed_event_months: 24, source_months: null, version: 3 }));

    expect(await closedField()).toHaveValue("24");
    expect(await closedField()).toBeDisabled();
    expect(screen.queryByRole("button", { name: "Speichern" })).not.toBeInTheDocument();
    expect(screen.getByText(/Nur die Organisationsleitung ändert die Fristen/)).toBeVisible();
  });

  it("owner only: a member sees no fields and loads nothing", async () => {
    const calls = setup("member");

    expect(await screen.findByText(/Nur die Organisationsleitung/)).toBeVisible();
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
    expect(calls).toHaveLength(0);
  });
});
