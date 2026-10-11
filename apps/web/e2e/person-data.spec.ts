import { expect, type Page, test } from "@playwright/test";
import { events, fakeSession, sessionWithRole } from "./fixtures";

// Invented data (doc/design/principles.md). The checks use a fake API; the Rust acceptance tests of
// `crates/tada/tests/person_features.rs` prove the server side of the same flows.
const PERSON = "0199b8e0-0000-7000-8000-0000000000d1";
const SOURCE_ITEM = "0199b8e0-0000-7000-8000-0000000000e1";
const ACTION = "0199b8e0-0000-7000-8000-0000000000e2";
const CHANGESET = "0199b8e0-0000-7000-8000-000000000c01";
const event = events[0];

const json = (body: unknown, status = 200) => ({
  status,
  contentType: "application/json",
  body: JSON.stringify(body),
});

test("an owner finds the export, redacts a mention and sees it removed", async ({ page }) => {
  await fakeSession(page, sessionWithRole("owner"));
  await page.route("**/api/v1/persons**", (route) => {
    const path = new URL(route.request().url()).pathname;
    if (path.endsWith("/mentions")) {
      return route.fulfill(json({ items: mentions }));
    }
    if (path.endsWith("/export")) {
      return route.fulfill({
        ...json({ format_version: 1 }),
        headers: { "Content-Disposition": 'attachment; filename="person-PER-001.json"' },
      });
    }
    return route.fulfill(
      json({
        items: [
          {
            id: PERSON,
            local_id: "PER-001",
            name: "Beat Muster",
            version: 1,
            can_change: true,
          },
        ],
      }),
    );
  });
  const mentions = [
    {
      kind: "source-item",
      id: SOURCE_ITEM,
      event_id: event?.id,
      excerpt: "Beat Muster kommt im Mai 2030.",
      redacted: false,
      disputed: false,
    },
    {
      kind: "action",
      id: ACTION,
      event_id: event?.id,
      excerpt: "Anruf bei Beat Muster",
      redacted: false,
      disputed: false,
    },
  ];
  let redactions: unknown;
  await page.route("**/api/v1/redactions", (route) => {
    redactions = route.request().postDataJSON();
    mentions[0] = { ...mentions[0], excerpt: "", redacted: true } as (typeof mentions)[0];
    return route.fulfill(json({ applied: ["0199b8e0-0000-7000-8000-0000000000f1"] }, 201));
  });

  await page.goto("/persons");
  await page.getByRole("button", { name: /bearbeiten$/ }).click();
  await expect(page.getByRole("heading", { name: "Datenschutz" })).toBeVisible();

  // The page loads the export and saves it under the name of the server; the Rust test checks the content.
  const download = page.waitForEvent("download");
  await page.getByRole("button", { name: "Export herunterladen" }).click();
  expect((await download).suggestedFilename()).toBe("person-PER-001.json");

  await page.locator("label", { hasText: "Beat Muster kommt im Mai 2030" }).click();
  await page.getByRole("button", { name: "Entfernen" }).click();
  await page.getByRole("button", { name: "Endgültig entfernen" }).click();

  expect(redactions).toEqual({
    targets: [{ kind: "source-item", id: SOURCE_ITEM }],
    reason: "request",
  });
  await expect(page.getByText("1 Eintrag entfernt.")).toBeVisible();
  await expect(page.getByText("entfernt", { exact: true })).toBeVisible();
  await expect(page.getByText("Beat Muster kommt im Mai 2030.")).toHaveCount(0);
});

/** The fake API of one event and one open changeset whose evidence is removed. */
async function fakeClosableEvent(page: Page): Promise<void> {
  let closed = false;
  const current = () => ({
    ...event,
    status: closed ? "closed" : "active",
    ...(closed && { closed_at: "2030-07-01T10:00:00Z" }),
    version: closed ? 2 : 1,
  });
  await fakeSession(page, sessionWithRole("member"));
  await page.route("**/api/v1/events/*/memberships", (route) => route.fulfill(json({ items: [] })));
  await page.route("**/api/v1/events/*/profile", (route) =>
    route.fulfill(json({ facts: [], proposals: [], open_questions: [] })),
  );
  await page.route("**/api/v1/events/*/fields", (route) => route.fulfill(json({ items: [] })));
  await page.route("**/api/v1/events/*/close", (route) => {
    closed = true;
    return route.fulfill(json(current()));
  });
  await page.route(/\/api\/v1\/events\/[^/]+$/, (route) => route.fulfill(json(current())));
  await page.route(/\/api\/v1\/changesets\/[^/]+$/, (route) =>
    route.fulfill(
      json({
        id: CHANGESET,
        event_id: event?.id,
        author: { kind: "ai", id: "0199b8e0-0000-7000-8000-0000000000c1", channel: "api-token" },
        source_version_id: SOURCE_ITEM,
        created_at: "2030-05-18T09:00:00Z",
        redacted_proposals: [],
        proposals: [
          {
            id: "0199b8e0-0000-7000-8000-0000000001a1",
            depends_on: [],
            reason: "Die Nachricht nennt den Ort.",
            evidence: [
              {
                source_version_id: SOURCE_ITEM,
                passage: { start: 0, end: 10, quote: "" },
                excerpt: { before: "", quote: "", after: "", redacted: true },
              },
            ],
            status: "open",
            stale: false,
            overdue: false,
            routed_to_me: true,
            can_review: true,
            operation: {
              kind: "create-open-question",
              id: "0199b8e0-0000-7000-8000-0000000004a9",
              event_id: event?.id,
              text: "Wer schliesst den Hangar ab?",
              owner: "0199b8e0-0000-7000-8000-0000000000b1",
            },
          },
        ],
      }),
    ),
  );
}

test("an event manager closes an event and the review buttons are gone", async ({ page }) => {
  await fakeClosableEvent(page);

  await page.goto(`/inbox/${CHANGESET}`);
  await expect(page.getByText("entfernt", { exact: true }).first()).toBeVisible();
  await expect(page.getByRole("button", { name: "Annehmen" }).first()).toBeVisible();

  await page.goto(`/events/${event?.id}`);
  await page.getByRole("button", { name: "Anlass abschliessen" }).click();
  await page.getByRole("dialog").getByRole("button", { name: "Anlass abschliessen" }).click();
  await expect(page.getByText("Abgeschlossen am 01.07.2030")).toBeVisible();
  await expect(page.getByRole("button", { name: "Anlass abschliessen" })).toHaveCount(0);

  await page.goto(`/inbox/${CHANGESET}`);
  await expect(page.getByText(/Dieser Anlass ist abgeschlossen/)).toBeVisible();
  await expect(page.getByRole("button", { name: "Annehmen" })).toHaveCount(0);
});
