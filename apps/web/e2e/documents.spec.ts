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
const DOCUMENT_ID = "0199b8e0-0000-7000-8000-0000000000d1";
const sha = "0123456789abcdef".repeat(4);

// Invented documents with long German file names and umlauts (doc/design/principles.md).
function upload(number: number, fileName: string, mediaType: string) {
  return {
    id: `0199b8e0-0000-7000-8000-0000000000e${number}`,
    document_id: DOCUMENT_ID,
    number,
    kind: "upload",
    file_name: fileName,
    media_type: mediaType,
    size_bytes: 2_500_000,
    sha256: sha,
    uploaded_by: sessionInfo.user_id,
    source_version_id: `0199b8e0-0000-7000-8000-0000000000e${number}`,
    created_at: `2028-03-0${number}T13:12:00Z`,
  };
}

const draft = {
  id: "0199b8e0-0000-7000-8000-0000000000e9",
  document_id: DOCUMENT_ID,
  number: 3,
  kind: "draft",
  status: "review",
  sha256: sha,
  uploaded_by: "0199b8e0-0000-7000-8000-0000000000b2",
  created_at: "2028-03-05T09:00:00Z",
};

function document(id: string, readableId: string, name: string, newest: unknown) {
  return {
    id,
    event_id: event?.id,
    readable_id: readableId,
    name,
    owner: sessionInfo.user_id,
    created_at: "2028-03-01T13:12:00Z",
    version: 1,
    newest_version: newest,
  };
}

const long = "Veranstaltungsbewilligungsverfahren-Unterlagen Flugtag Übersicht 2028.pdf";
const documents = [
  document(DOCUMENT_ID, "DOC-001", long, upload(2, long, "application/pdf")),
  document(
    "0199b8e0-0000-7000-8000-0000000000d2",
    "DOC-002",
    "Notizen.txt",
    upload(1, "Notizen.txt", "text/plain; charset=utf-8"),
  ),
];

async function fakeDocuments(page: Page, items: unknown[]): Promise<void> {
  await fakeEvent(page, event);
  await page.route("**/api/v1/events/*/documents*", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ items }),
    }),
  );
}

const VENUE_FIELD = "0199b8e0-0000-7000-8000-0000000000f2";
const VENUE_FACT = "0199b8e0-0000-7000-8000-0000000000a2";
const SOURCE_VERSION = "0199b8e0-0000-7000-8000-0000000000e1";
const venueLink = `tada:fact/${VENUE_FACT}?v=2`;
const sourceLink = `tada:source/${SOURCE_VERSION}#0-40`;
const hiddenLink = "tada:fact/0199b8e0-0000-7000-8000-0000000000a3?v=1";

// A draft with long German words, a cited fact, a source and a target that the reader cannot see.
const rendering = {
  markdown: [
    "# Konzept Flugtag Veranstaltungsbewilligungsverfahren",
    `Der Anlass findet auf dem Flugplatz [](${venueLink}) statt.`,
    `Die Besucherzahl stammt aus [dem Protokoll der Vorstandssitzung](${sourceLink}).`,
    `Das Konto lautet [](${hiddenLink}).`,
    "Es kommen etwa 500 Gäste.",
  ].join("\n\n"),
  lint_warnings: [],
  links: {
    [venueLink]: {
      kind: "fact",
      fact_id: VENUE_FACT,
      version: 2,
      state: "assumption",
      value: { type: "text", text: "Testwil" },
    },
    [sourceLink]: {
      kind: "source",
      source_version_id: SOURCE_VERSION,
      passage: { start: 0, end: 40, quote: "Wir erwarten etwa 500 Gäste am Samstag." },
    },
    [hiddenLink]: { kind: "hidden" },
  },
};

const venueFact = {
  id: VENUE_FACT,
  field_id: VENUE_FIELD,
  field_key: "venue",
  state: "assumption",
  value: { type: "text", text: "Testwil" },
  version: 2,
  evidence: [],
  accepted_by: { kind: "member", id: sessionInfo.user_id, channel: "web" },
  accepted_at: "2028-03-02T08:00:00Z",
};

const venueField = {
  id: VENUE_FIELD,
  key: "venue",
  label: { kind: "text", text: "Veranstaltungsort" },
  value_type: { type: "text" },
  value_schema: {},
  description: "",
  module: "core",
  status: "active",
};

async function fakeDocument(page: Page, versions: unknown[]): Promise<void> {
  await fakeEvent(page, event);
  await fakeProfile(page, { facts: [venueFact], proposals: [], open_questions: [] }, [venueField]);
  await page.route("**/api/v1/events/*/memberships", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ items: [] }),
    }),
  );
  await page.route("**/api/v1/document-versions/*/rendering", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ version: draft, draft: rendering }),
    }),
  );
  await page.route("**/api/v1/documents/*/versions", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ items: versions }),
    }),
  );
  await page.route(`**/api/v1/documents/${DOCUMENT_ID}`, (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ ...documents[0], version: 3, facts_changed: true }),
    }),
  );
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
  await page.route("**/api/v1/document-versions/*/content*", (route) =>
    route.fulfill({
      status: 200,
      contentType: "text/plain; charset=utf-8",
      body: "Notizen zum Flugtag",
    }),
  );
}

for (const viewport of viewports) {
  for (const theme of themes) {
    test(`documents list, ${theme}, ${viewport.name} px: no axe violation, screenshot`, async ({
      page,
    }) => {
      await page.setViewportSize(viewport);
      await fakeSession(page);
      await fakeDocuments(page, documents);
      await page.goto(`/events/${event?.id}/documents`);
      await setTheme(page, theme);
      await expect(page.getByRole("table", { name: "Dokumente" })).toBeVisible();

      const results = await new AxeBuilder({ page })
        .withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"])
        .analyze();
      expect(results.violations).toEqual([]);

      await fontsLoaded(page);
      await expect(page).toHaveScreenshot(`documents-${theme}-${viewport.name}.png`, {
        fullPage: true,
      });
    });

    test(`document versions, ${theme}, ${viewport.name} px: no axe violation, screenshot`, async ({
      page,
    }) => {
      await page.setViewportSize(viewport);
      await fakeSession(page);
      await fakeDocument(page, [
        upload(1, "Notizen.txt", "text/plain; charset=utf-8"),
        upload(2, "Notizen neu.txt", "text/plain; charset=utf-8"),
        draft,
      ]);
      await page.goto(`/documents/${DOCUMENT_ID}`);
      await setTheme(page, theme);
      await expect(page.getByRole("table", { name: "Versionen" })).toBeVisible();
      await expect(page.getByText("Anna Muster").first()).toBeVisible();
      // The newest version is the draft: the preview renders it, and the manager can approve it.
      await expect(page.getByRole("button", { name: /Beleg zu Veranstaltungsort/ })).toBeVisible();
      await expect(page.getByText("entfernt")).toBeVisible();
      await expect(page.getByText("Fakten geändert")).toBeVisible();
      await expect(page.getByRole("button", { name: "Version freigeben" })).toBeVisible();

      const results = await new AxeBuilder({ page })
        .withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"])
        .analyze();
      expect(results.violations).toEqual([]);

      await fontsLoaded(page);
      await expect(page).toHaveScreenshot(`document-${theme}-${viewport.name}.png`, {
        fullPage: true,
      });
    });
  }
}

test("a member uploads a file and sees the failure of the server", async ({ page }) => {
  await fakeSession(page);
  await fakeDocuments(page, documents);
  await page.route("**/api/v1/events/*/documents", async (route) => {
    if (route.request().method() === "POST") {
      await route.fulfill({
        status: 413,
        contentType: "application/problem+json",
        body: JSON.stringify({
          type: "https://github.com/zarubaf/tada/blob/main/doc/problems.md#payload-too-large",
          code: "payload-too-large",
          title: "The body is too large.",
          status: 413,
          instance: "urn:uuid:01a1118e-3359-73dd-a500-feed65806a9d",
          request_id: "01a1118e-3359-73dd-a500-feed65806a9d",
        }),
      });
    } else {
      await route.fallback();
    }
  });
  await page.goto(`/events/${event?.id}/documents`);
  await expect(page.getByRole("table", { name: "Dokumente" })).toBeVisible();

  // The member pressed the button; the picker returns focus to it.
  await page.getByRole("button", { name: "Datei hochladen" }).focus();
  await page.locator("input[type=file]").setInputFiles({
    name: "gross.pdf",
    mimeType: "application/pdf",
    buffer: Buffer.from("%PDF-1.4"),
  });

  await expect(page.getByRole("main").getByRole("alert")).toHaveText("Die Datei ist zu gross.");
  await expect(page.getByRole("button", { name: "Datei hochladen" })).toBeFocused();
});

// The comparison of two drafts (ADR 0051).
const earlierDraft = { ...draft, id: "0199b8e0-0000-7000-8000-0000000000e8", number: 2 };
const diff = {
  lines: [
    { kind: "unchanged", old_line: 1, new_line: 1, text: "# Konzept Flugtag" },
    {
      kind: "removed",
      old_line: 2,
      text: "Es kommen etwa 400 Gäste zum Veranstaltungsbewilligungsverfahren.",
    },
    {
      kind: "added",
      new_line: 2,
      text: "Es kommen etwa 500 Gäste zum Veranstaltungsbewilligungsverfahren.",
    },
  ],
  facts: {
    changed: [{ fact_id: VENUE_FACT, from: 1, to: 2 }],
    added: [{ fact_id: "0199b8e0-0000-7000-8000-0000000000a3", version: 1 }],
    removed: [],
  },
};

for (const viewport of viewports) {
  for (const theme of themes) {
    test(`document differences, ${theme}, ${viewport.name} px: no axe violation, screenshot`, async ({
      page,
    }) => {
      await page.setViewportSize(viewport);
      await fakeSession(page);
      await fakeDocument(page, [earlierDraft, draft]);
      await page.route("**/api/v1/documents/*/diff*", (route) =>
        route.fulfill({
          status: 200,
          contentType: "application/json",
          body: JSON.stringify(diff),
        }),
      );
      await page.goto(`/documents/${DOCUMENT_ID}/diff?from=${earlierDraft.id}&to=${draft.id}`);
      await setTheme(page, theme);
      await expect(page.getByRole("table", { name: "Zeilen" })).toBeVisible();
      await expect(page.getByText("Veranstaltungsort: Version 1 zu Version 2")).toBeVisible();

      const results = await new AxeBuilder({ page })
        .withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"])
        .analyze();
      expect(results.violations).toEqual([]);

      await fontsLoaded(page);
      await expect(page).toHaveScreenshot(`document-diff-${theme}-${viewport.name}.png`, {
        fullPage: true,
      });
    });
  }
}

test("a member opens the evidence of a cited fact and focus returns to it", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await fakeSession(page);
  await fakeDocument(page, [upload(1, "Notizen.txt", "text/plain; charset=utf-8"), draft]);
  await page.goto(`/documents/${DOCUMENT_ID}`);

  const button = page.getByRole("button", { name: /Beleg zu Veranstaltungsort/ });
  await button.click();
  await expect(page.getByRole("dialog", { name: "Veranstaltungsort" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(button).toBeFocused();
});

// ADR 0024: German text can be 40 % longer than the source. No text may overflow its box.
for (const viewport of viewports) {
  test(`pseudo-locale, documents, ${viewport.name} px: no text overflows`, async ({ page }) => {
    await page.setViewportSize(viewport);
    await fakeSession(page);
    await fakeDocuments(page, documents);
    await page.goto(`/events/${event?.id}/documents?pseudo`);
    await expect(page.getByRole("table")).toBeVisible();
    expect(await textOverflows(page)).toEqual([]);
  });

  test(`pseudo-locale, document, ${viewport.name} px: no text overflows`, async ({ page }) => {
    await page.setViewportSize(viewport);
    await fakeSession(page);
    await fakeDocument(page, [upload(1, "Notizen.txt", "text/plain; charset=utf-8"), draft]);
    await page.goto(`/documents/${DOCUMENT_ID}?pseudo`);
    await expect(page.getByRole("table")).toBeVisible();
    expect(await textOverflows(page)).toEqual([]);
  });
}
