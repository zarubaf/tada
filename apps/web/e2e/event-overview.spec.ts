import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";
import {
  events,
  fakeEvent,
  fakeProfile,
  fakeSession,
  fontsLoaded,
  sessionInfo,
  setTheme,
  textOverflows,
  themes,
  viewports,
} from "./fixtures";

const event = events[0];

function field(key: string, valueType: unknown) {
  return {
    id: `0199b8e0-0000-7000-8000-0000000001${key.length}`,
    key,
    label: { kind: "message", id: `field-${key}` },
    value_type: valueType,
    value_schema: {},
    description: "",
    module: "core",
    status: "active",
  };
}

// Invented fields, facts and questions with long German words (doc/design/principles.md).
const fields = [
  field("date_window", { type: "date-window", granularity: null }),
  field("duration_days", { type: "quantity", unit: "day" }),
  field("visitor_estimate", { type: "quantity", unit: "person_per_day" }),
  field("entry_fee_adult", { type: "money", currency: "CHF" }),
  field("venue", { type: "text" }),
  field("exact_dates", { type: "date-window", granularity: "day" }),
];
const idOf = (key: string) => fields.find((candidate) => candidate.key === key)?.id;

const author = { kind: "member", id: sessionInfo.user_id, channel: "web" };

function fact(key: string, state: string, value: unknown, extra: object = {}) {
  return {
    id: `0199b8e0-0000-7000-8000-0000000002${key.length}`,
    field_id: idOf(key),
    field_key: key,
    state,
    value,
    version: 1,
    evidence: [],
    accepted_by: author,
    accepted_at: "2028-03-04T08:30:00Z",
    ...extra,
  };
}

const profile = {
  facts: [
    fact(
      "date_window",
      "accepted",
      { type: "date-window", start: "2028-06-01", end: "2028-06-30", granularity: "month" },
      {
        evidence: [
          {
            source_version_id: "0199b8e0-0000-7000-8000-0000000000e2",
            passage: {
              start: 40,
              end: 61,
              quote: "Das Fly-in findet im Juni 2028 statt.",
              page: 2,
            },
            captured_at: "2028-03-03T12:12:00Z",
          },
        ],
      },
    ),
    fact("venue", "accepted", { type: "text", text: "Flugplatz Musterhausen-Obersteinfelden" }),
    fact(
      "visitor_estimate",
      "assumption",
      { type: "quantity", min: "18000", max: "22000" },
      {
        approximate: true,
      },
    ),
    fact("entry_fee_adult", "unknown", undefined),
  ],
  proposals: [
    {
      id: "0199b8e0-0000-7000-8000-0000000003a1",
      changeset_id: "0199b8e0-0000-7000-8000-0000000003c1",
      field_id: idOf("entry_fee_adult"),
      state: "accepted",
      value: { type: "money", min: 1500, max: 1500 },
      created_at: "2028-03-05T09:00:00Z",
    },
    {
      id: "0199b8e0-0000-7000-8000-0000000003a2",
      changeset_id: "0199b8e0-0000-7000-8000-0000000003c2",
      field_id: idOf("duration_days"),
      state: "assumption",
      value: { type: "quantity", min: "1", max: "1" },
      created_at: "2028-03-05T10:00:00Z",
    },
  ],
  open_questions: [
    {
      id: "0199b8e0-0000-7000-8000-0000000004a1",
      local_id: "QST-001",
      text: "Wer übernimmt die Verpflegung der Besucherinnen und Besucher am Samstag?",
      owner_id: sessionInfo.user_id,
      version: 1,
    },
    {
      id: "0199b8e0-0000-7000-8000-0000000004a2",
      local_id: "QST-002",
      text: "Reicht die Veranstaltungsbewilligung für beide Tage?",
      owner_id: sessionInfo.user_id,
      version: 1,
    },
  ],
};

async function fakeOverview(page: Page): Promise<void> {
  await fakeSession(page);
  await fakeEvent(page, event);
  await fakeProfile(page, profile, fields);
  await page.route("**/api/v1/members*", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        items: [
          { user_id: sessionInfo.user_id, display_name: "Anna Muster", role: "member", version: 1 },
        ],
      }),
    }),
  );
  await page.route("**/api/v1/events/*/documents*", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: '{"items":[]}' }),
  );
}

for (const viewport of viewports) {
  for (const theme of themes) {
    test(`event overview, ${theme}, ${viewport.name} px: no axe violation, screenshot`, async ({
      page,
    }) => {
      await page.setViewportSize(viewport);
      await fakeOverview(page);
      await page.goto(`/events/${event?.id}`);
      await setTheme(page, theme);
      await expect(page.getByRole("heading", { level: 2, name: "Fakten" })).toBeVisible();
      await expect(page.getByText("Vorschlag", { exact: true }).first()).toBeVisible();

      const results = await new AxeBuilder({ page })
        .withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"])
        .analyze();
      expect(results.violations).toEqual([]);
      await fontsLoaded(page);
      await expect(page).toHaveScreenshot(`event-overview-${theme}-${viewport.name}.png`, {
        fullPage: true,
      });
    });

    test(`evidence panel, ${theme}, ${viewport.name} px: no axe violation, screenshot`, async ({
      page,
    }) => {
      await page.setViewportSize(viewport);
      await fakeOverview(page);
      await page.goto(`/events/${event?.id}`);
      await setTheme(page, theme);
      await page.getByRole("button", { name: "Beleg zu Zeitfenster" }).click();
      const dialog = page.getByRole("dialog", { name: "Zeitfenster" });
      await expect(dialog).toBeVisible();
      await expect(dialog.getByText("Anna Muster")).toBeVisible();

      const results = await new AxeBuilder({ page })
        .withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"])
        .analyze();
      expect(results.violations).toEqual([]);
      await fontsLoaded(page);
      await expect(page).toHaveScreenshot(`evidence-panel-${theme}-${viewport.name}.png`);
    });
  }

  // ADR 0024: German text can be 40 % longer than the source. No text may overflow its box.
  test(`pseudo-locale, event overview, ${viewport.name} px: no text overflows`, async ({
    page,
  }) => {
    await page.setViewportSize(viewport);
    await fakeOverview(page);
    await page.goto(`/events/${event?.id}?pseudo`);
    await expect(page.getByRole("heading", { level: 2 }).first()).toBeVisible();
    expect(await textOverflows(page)).toEqual([]);
  });
}
