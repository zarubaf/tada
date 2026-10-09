import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";
import {
  events,
  fakeSession,
  fontsLoaded,
  sessionWithRole,
  setTheme,
  textOverflows,
  themes,
  viewports,
} from "./fixtures";

const event = events[0];
const ai = { kind: "ai", id: "0199b8e0-0000-7000-8000-0000000000c1", channel: "api-token" };

const CS_OLD = "0199b8e0-0000-7000-8000-000000000c01";
const CS_NEW = "0199b8e0-0000-7000-8000-000000000c02";
const CS_ORG = "0199b8e0-0000-7000-8000-000000000c03";
const VENUE = "0199b8e0-0000-7000-8000-0000000000f1";
const NEW_FIELD = "0199b8e0-0000-7000-8000-0000000000f2";
const P_FIELD = "0199b8e0-0000-7000-8000-0000000001a2";
const P_FACT = "0199b8e0-0000-7000-8000-0000000001a3";
const SOURCE = "0199b8e0-0000-7000-8000-0000000000e1";
const CS_DRAFT = "0199b8e0-0000-7000-8000-000000000c04";
const DRAFT_FACT = "0199b8e0-0000-7000-8000-0000000002a2";
const draftFactLink = `tada:fact/${DRAFT_FACT}?v=2`;
const draftHiddenLink = "tada:fact/0199b8e0-0000-7000-8000-0000000002a3?v=1";

// Invented data with long German words and umlauts (doc/design/principles.md).
const evidence = {
  source_version_id: SOURCE,
  passage: { start: 16, end: 58, quote: "Wir treffen uns auf dem Flugplatz Testwil." },
  excerpt: {
    before: "Hallo zusammen, die Besprechung der Veranstaltungsbewilligung ist erledigt. ",
    quote: "Wir treffen uns auf dem Flugplatz Testwil.",
    after: " Bis bald und viele Grüsse aus dem Vorstand.",
  },
};

function proposal(id: string, extra: object) {
  return {
    id,
    depends_on: [],
    reason: "Die E-Mail von Anna nennt den Ort des Anlasses.",
    evidence: [evidence],
    status: "open",
    stale: false,
    overdue: false,
    routed_to_me: true,
    ...extra,
  };
}

const changesets: Record<string, unknown> = {
  [CS_OLD]: {
    id: CS_OLD,
    event_id: event?.id,
    author: ai,
    source_version_id: SOURCE,
    created_at: "2028-02-01T09:00:00Z",
    proposals: [
      proposal("0199b8e0-0000-7000-8000-0000000001a1", {
        stale: true,
        operation: {
          kind: "set-fact",
          event_id: event?.id,
          field_id: VENUE,
          state: "accepted",
          value: { type: "text", text: "Flugplatz Testwil" },
          expected_version: 1,
        },
        current: {
          fact_id: "0199b8e0-0000-7000-8000-0000000002a1",
          version: 2,
          state: "accepted",
          value: { type: "text", text: "Flugplatz Musterhausen-Obersteinfelden" },
        },
      }),
    ],
  },
  [CS_NEW]: {
    id: CS_NEW,
    event_id: event?.id,
    author: ai,
    source_version_id: SOURCE,
    created_at: "2028-03-05T09:00:00Z",
    proposals: [
      proposal(P_FIELD, {
        operation: {
          kind: "add-field-definition",
          id: NEW_FIELD,
          event_id: event?.id,
          key: "hangar",
          label: { kind: "text", text: "Hangar" },
          value_type: { type: "text" },
          description: "Der Hangar des Anlasses.",
          module: "core",
        },
      }),
      proposal(P_FACT, {
        depends_on: [P_FIELD],
        operation: {
          kind: "set-fact",
          event_id: event?.id,
          field_id: NEW_FIELD,
          state: "assumption",
          value: { type: "text", text: "Hangar 3" },
          expected_version: null,
        },
      }),
      proposal("0199b8e0-0000-7000-8000-0000000001a4", {
        operation: {
          kind: "create-open-question",
          id: "0199b8e0-0000-7000-8000-0000000004a9",
          event_id: event?.id,
          text: "Wer schliesst den Hangar nach dem Anlass ab?",
          owner: "0199b8e0-0000-7000-8000-0000000000b1",
        },
      }),
    ],
  },
  // A draft with a cited fact, a target that the reader cannot see and two lint warnings.
  [CS_DRAFT]: {
    id: CS_DRAFT,
    event_id: event?.id,
    author: ai,
    source_version_id: SOURCE,
    created_at: "2028-03-07T09:00:00Z",
    proposals: [
      proposal("0199b8e0-0000-7000-8000-0000000001c1", {
        operation: {
          kind: "create-document-draft",
          event_id: event?.id,
          markdown: "Entwurf",
          document: {
            kind: "new",
            id: "0199b8e0-0000-7000-8000-0000000000d3",
            name: "Konzept Flugtag Veranstaltungsbewilligungsverfahren",
          },
        },
        draft: {
          markdown: [
            `Der Anlass findet auf dem Flugplatz [](${draftFactLink}) statt.`,
            "Es kommen etwa 500 Gäste.",
            `Das Konto lautet [](${draftHiddenLink}).`,
          ].join("\n\n"),
          lint_warnings: [
            { line: 3, kind: "number" },
            { line: 3, kind: "raw-html" },
          ],
          links: {
            [draftFactLink]: {
              kind: "fact",
              fact_id: DRAFT_FACT,
              version: 2,
              state: "assumption",
              value: { type: "text", text: "Testwil" },
            },
            [draftHiddenLink]: { kind: "hidden" },
          },
        },
      }),
    ],
  },
  [CS_ORG]: {
    id: CS_ORG,
    event_id: null,
    author: ai,
    source_version_id: SOURCE,
    created_at: "2028-03-06T09:00:00Z",
    proposals: [
      proposal("0199b8e0-0000-7000-8000-0000000001b1", {
        operation: {
          kind: "create-event",
          id: "0199b8e0-0000-7000-8000-000000000009",
          key: "TEST30",
          name: "Tag der offenen Tür Testwil",
          time_zone: "Europe/Zurich",
        },
      }),
    ],
  },
};

const CS_WORK = "0199b8e0-0000-7000-8000-000000000c06";
const NEW_PERSON = "0199b8e0-0000-7000-8000-0000000005a1";

// A new person who may exist already, a commitment with a condition and a change for another reviewer.
changesets[CS_WORK] = {
  id: CS_WORK,
  event_id: event?.id,
  author: ai,
  source_version_id: SOURCE,
  created_at: "2028-03-07T09:00:00Z",
  proposals: [
    proposal("0199b8e0-0000-7000-8000-0000000001d1", {
      overdue: true,
      duplicates: [
        {
          id: "0199b8e0-0000-7000-8000-0000000005a2",
          local_id: "PER-004",
          name: "Hans Beispiel",
          kind: "person",
        },
      ],
      operation: {
        kind: "create-person",
        id: NEW_PERSON,
        name: "Hans Beispiel",
        email: "hans@example.org",
        phone: null,
      },
    }),
    proposal("0199b8e0-0000-7000-8000-0000000001d2", {
      depends_on: ["0199b8e0-0000-7000-8000-0000000001d1"],
      operation: {
        kind: "create-commitment",
        id: "0199b8e0-0000-7000-8000-0000000005b1",
        event_id: event?.id,
        text: "Hans stellt den Hangar bereit.",
        promisor: { person: NEW_PERSON },
        owner: "0199b8e0-0000-7000-8000-0000000000b1",
        workstream: null,
        due_date: "2028-04-01",
        condition: "Wenn die Gemeinde zustimmt",
      },
    }),
    proposal("0199b8e0-0000-7000-8000-0000000001d3", {
      routed_to_me: false,
      operation: {
        kind: "change-action-status",
        event_id: event?.id,
        action_id: "0199b8e0-0000-7000-8000-0000000005c1",
        status: "blocked",
        expected_version: 1,
      },
    }),
  ],
};

const items = [
  {
    id: CS_OLD,
    event_id: event?.id,
    author: ai,
    created_at: "2028-02-01T09:00:00Z",
    open_proposals: 1,
    stale: true,
  },
  {
    id: CS_NEW,
    event_id: event?.id,
    author: ai,
    created_at: "2028-03-05T09:00:00Z",
    open_proposals: 3,
    stale: false,
  },
  {
    id: CS_ORG,
    event_id: null,
    author: ai,
    created_at: "2028-03-06T09:00:00Z",
    open_proposals: 1,
    stale: false,
  },
];

const venueField = {
  id: VENUE,
  key: "venue",
  label: { kind: "text", text: "Veranstaltungsort" },
  value_type: { type: "text" },
  value_schema: {},
  description: "",
  module: "core",
  status: "active",
};

function json(body: unknown) {
  return { status: 200, contentType: "application/json", body: JSON.stringify(body) };
}

/** The fake API of the Review Inbox. It returns the bodies of the applies that the page sent. */
async function fakeInbox(page: Page): Promise<unknown[]> {
  const applies: unknown[] = [];
  await fakeSession(page, sessionWithRole("admin"));
  await page.route("**/api/v1/events?*", (route) => route.fulfill(json({ items: events })));
  await page.route("**/api/v1/events/*/fields", (route) =>
    route.fulfill(json({ items: [venueField] })),
  );
  await page.route("**/api/v1/events/*/profile", (route) =>
    route.fulfill(json({ facts: [], proposals: [], open_questions: [] })),
  );
  await page.route(/\/api\/v1\/changesets\?/, (route) => route.fulfill(json({ items })));
  await page.route(/\/api\/v1\/changesets\/[^/]+$/, (route) => {
    const id = route.request().url().split("/").pop() as string;
    return route.fulfill(json(changesets[id]));
  });
  await page.route(/\/api\/v1\/changesets\/[^/]+\/apply$/, (route) => {
    applies.push(route.request().postDataJSON());
    return route.fulfill(
      json({ proposals: [{ id: P_FIELD, status: "accepted" }], open_questions: [], documents: [] }),
    );
  });
  return applies;
}

const states = [
  { name: "list", path: "/inbox" },
  { name: "detail", path: `/inbox/${CS_NEW}` },
  { name: "conflict", path: `/inbox/${CS_OLD}` },
  { name: "draft", path: `/inbox/${CS_DRAFT}` },
  { name: "work", path: `/inbox/${CS_WORK}` },
] as const;

for (const viewport of viewports) {
  for (const theme of themes) {
    for (const state of states) {
      test(`inbox, ${state.name}, ${theme}, ${viewport.name} px: no axe violation, screenshot`, async ({
        page,
      }) => {
        await page.setViewportSize(viewport);
        await fakeInbox(page);
        await page.goto(state.path);
        await setTheme(page, theme);
        await expect(page.getByRole("heading", { level: 1, name: "Eingang" })).toBeVisible();
        if (state.name === "list") {
          await expect(page.getByRole("link", { name: /Veraltet/ })).toBeVisible();
        } else {
          await expect(page.getByRole("article").first()).toBeVisible();
        }

        const results = await new AxeBuilder({ page })
          .withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"])
          .analyze();
        expect(results.violations).toEqual([]);
        await fontsLoaded(page);
        await expect(page).toHaveScreenshot(`inbox-${state.name}-${theme}-${viewport.name}.png`, {
          fullPage: true,
        });
      });
    }

    test(`inbox, edit form and selection, ${theme}, ${viewport.name} px: no axe violation, screenshot`, async ({
      page,
    }) => {
      await page.setViewportSize(viewport);
      await fakeInbox(page);
      await page.goto(`/inbox/${CS_NEW}`);
      await setTheme(page, theme);
      await page.locator("label").filter({ hasText: "Offene Frage anlegen auswählen" }).click();
      const card = page.getByRole("article", { name: "Wert für „Hangar“" });
      await card.getByRole("button", { name: /^Bearbeiten und annehmen/ }).click();
      await expect(card.getByRole("textbox", { name: "Text" })).toBeFocused();

      const results = await new AxeBuilder({ page })
        .withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"])
        .analyze();
      expect(results.violations).toEqual([]);
      await fontsLoaded(page);
      await expect(page).toHaveScreenshot(`inbox-edit-${theme}-${viewport.name}.png`, {
        fullPage: true,
      });
    });
  }

  // ADR 0024: German text can be 40 % longer than the source. No text may overflow its box.
  test(`pseudo-locale, inbox, ${viewport.name} px: no text overflows`, async ({ page }) => {
    await page.setViewportSize(viewport);
    await fakeInbox(page);
    await page.goto(`/inbox/${CS_NEW}?pseudo`);
    await expect(page.getByRole("article").first()).toBeVisible();
    expect(await textOverflows(page)).toEqual([]);
  });
}

test("inbox keyboard flow: J and K move, A applies, typing does not trigger a key", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const applies = await fakeInbox(page);
  await page.goto(`/inbox/${CS_OLD}`);
  await expect(page.getByRole("article").first()).toBeVisible();

  // J moves to the next changeset and focus stays in the list.
  await page.getByRole("heading", { level: 1, name: "Eingang" }).focus();
  await page.keyboard.press("j");
  await expect(page).toHaveURL(new RegExp(`/inbox/${CS_NEW}$`));
  await expect(page.getByRole("link", { name: /3 offene Vorschläge/ })).toBeFocused();
  await page.keyboard.press("k");
  await expect(page).toHaveURL(new RegExp(`/inbox/${CS_OLD}$`));
  await page.keyboard.press("j");
  await expect(page.getByRole("article", { name: "Feld „Hangar“ hinzufügen" })).toBeVisible();

  // E opens the form of the active proposal. A key typed in the field is text, not a shortcut.
  const card = page.getByRole("article", { name: "Wert für „Hangar“" });
  await card.getByText("Hangar 3").click();
  await page.keyboard.press("e");
  const field = card.getByRole("textbox", { name: "Text" });
  await expect(field).toBeFocused();
  await page.keyboard.type("area");
  await expect(field).toHaveValue("Hangar 3area");
  expect(applies).toHaveLength(0);

  // A on the active proposal applies it with its dependency.
  await card.getByRole("button", { name: "Abbrechen" }).click();
  await expect(card.getByRole("button", { name: /^Bearbeiten und annehmen/ })).toBeFocused();
  await card.getByText("Hangar 3").click();
  await page.keyboard.press("a");
  await expect(page.getByText("1 Vorschlag angenommen.")).toBeVisible();
  expect(applies).toEqual([
    { selected: expect.arrayContaining([P_FACT, P_FIELD]), edits: [], links: [] },
  ]);
});
