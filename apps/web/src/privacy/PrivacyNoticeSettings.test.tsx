import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it } from "vitest";
import { type Api, createApi } from "../api/client";
import { Router } from "../router/Router";
import { LiveRegion } from "../ui/LiveRegion";
import { PrivacyNoticeSettings } from "./PrivacyNoticeSettings";

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
function Harness({ api, isOwner }: { api: Api; isOwner: boolean }) {
  const [status, setStatus] = useState<string>();
  const [failure, setFailure] = useState<string>();
  return (
    <Router>
      <LiveRegion kind="alert">{failure}</LiveRegion>
      <LiveRegion kind="status">{status}</LiveRegion>
      <PrivacyNoticeSettings
        api={api}
        isOwner={isOwner}
        onStatus={setStatus}
        onFailure={setFailure}
      />
    </Router>
  );
}

const SET = "POST /api/v1/organization/privacy-notice/set";

function setup(isOwner: boolean, ...responses: (() => Response)[]) {
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
  render(<Harness api={api} isOwner={isOwner} />);
  return calls;
}

const user = userEvent.setup({ delay: null });
const field = () => screen.findByRole("textbox", { name: "Text der Datenschutzerklärung" });

describe("PrivacyNoticeSettings", () => {
  it("shows the template in the field while the organization has no own text", async () => {
    setup(true, () => json(200, { markdown: null, version: 1 }));

    const text = await field();
    expect((text as HTMLTextAreaElement).value).toContain("# Verantwortlich");
    expect(screen.getByText(/Es gilt die Vorlage/)).toBeVisible();
    expect(screen.queryByRole("button", { name: "Vorlage wiederherstellen" })).toBeNull();
  });

  it("saves the text with the version, keeps focus on the button and announces the result", async () => {
    const calls = setup(
      true,
      () => json(200, { markdown: "Alt", version: 2 }),
      () => json(200, { markdown: "Neu", version: 3 }),
    );
    const text = await field();
    await user.clear(text);
    await user.type(text, "Neu");
    const save = screen.getByRole("button", { name: "Speichern" });
    await user.click(save);

    expect(await screen.findByText("Datenschutzerklärung gespeichert.")).toBeTruthy();
    expect(calls.find((c) => c.call === SET)?.body).toEqual({
      markdown: "Neu",
      expected_version: 2,
    });
    expect(save).toHaveFocus();
  });

  it("sends the next save with the new version", async () => {
    const calls = setup(
      true,
      () => json(200, { markdown: "Alt", version: 2 }),
      () => json(200, { markdown: "Eins", version: 3 }),
      () => json(200, { markdown: "Zwei", version: 4 }),
    );
    const text = await field();
    await user.clear(text);
    await user.type(text, "Eins");
    await user.click(screen.getByRole("button", { name: "Speichern" }));
    await screen.findByText("Datenschutzerklärung gespeichert.");
    await user.type(text, "x");
    await user.click(screen.getByRole("button", { name: "Speichern" }));

    await waitFor(() => expect(calls.filter((c) => c.call === SET)).toHaveLength(2));
    expect(calls.filter((c) => c.call === SET)[1]?.body).toMatchObject({ expected_version: 3 });
  });

  it("shows an invalid text at the field and moves focus to it", async () => {
    setup(
      true,
      () => json(200, { markdown: "Alt", version: 2 }),
      () => problem(422, "validation-failed", [{ pointer: "/markdown", code: "empty" }]),
    );
    const text = await field();
    await user.clear(text);
    await user.click(screen.getByRole("button", { name: "Speichern" }));

    expect(await screen.findByText("Geben Sie einen Text ein.")).toBeTruthy();
    await waitFor(() => expect(text).toHaveFocus());
  });

  it("keeps the text and the button after a version conflict and announces the failure", async () => {
    setup(
      true,
      () => json(200, { markdown: "Alt", version: 2 }),
      () => problem(409, "record-version-conflict"),
    );
    const text = await field();
    await user.type(text, " mehr");
    const save = screen.getByRole("button", { name: "Speichern" });
    await user.click(save);

    expect(
      await screen.findByText(/Jemand hat die Datenschutzerklärung inzwischen geändert/),
    ).toBeTruthy();
    expect((text as HTMLTextAreaElement).value).toBe("Alt mehr");
    expect(save).toHaveFocus();
  });

  it("goes back to the template after a confirmation", async () => {
    const calls = setup(
      true,
      () => json(200, { markdown: "Eigener Text", version: 2 }),
      () => json(200, { markdown: null, version: 3 }),
    );
    await field();
    await user.click(screen.getByRole("button", { name: "Vorlage wiederherstellen" }));
    const dialog = await screen.findByRole("alertdialog");
    expect(calls.some((c) => c.call === SET)).toBe(false);
    await user.click(within(dialog).getByRole("button", { name: "Vorlage wiederherstellen" }));

    await waitFor(() =>
      expect(calls.find((c) => c.call === SET)?.body).toEqual({
        markdown: null,
        expected_version: 2,
      }),
    );
    expect(await screen.findByText("Es gilt wieder die Vorlage.")).toBeTruthy();
    expect(((await field()) as HTMLTextAreaElement).value).toContain("# Verantwortlich");
  });

  it("shows a member no field, only a link to the notice", async () => {
    const calls = setup(false);

    expect(screen.getByText(/Nur die Organisationsleitung/)).toBeVisible();
    expect(screen.getByRole("link", { name: "Datenschutzerklärung lesen" })).toHaveAttribute(
      "href",
      "/privacy",
    );
    expect(screen.queryByRole("textbox")).toBeNull();
    expect(calls).toEqual([]);
  });

  it("offers a retry when the text does not load", async () => {
    setup(
      true,
      () => problem(503, "unavailable"),
      () => json(200, { markdown: "Alt", version: 2 }),
    );
    await user.click(await screen.findByRole("button", { name: "Erneut versuchen" }));

    expect(await field()).toBeVisible();
  });
});
