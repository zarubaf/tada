import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";
import {
  events,
  fakeEvent,
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
    upload(1, "Notizen.txt", "text/plain"),
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

async function fakeDocument(page: Page, versions: unknown[]): Promise<void> {
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
      body: JSON.stringify(documents[0]),
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
    route.fulfill({ status: 200, contentType: "text/plain", body: "Notizen zum Flugtag" }),
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
        upload(1, "Notizen.txt", "text/plain"),
        upload(2, "Notizen neu.txt", "text/plain"),
        draft,
      ]);
      await page.goto(`/documents/${DOCUMENT_ID}`);
      await setTheme(page, theme);
      await expect(page.getByRole("table", { name: "Versionen" })).toBeVisible();
      await expect(page.getByText("Anna Muster").first()).toBeVisible();
      await expect(page.frameLocator("iframe").getByText("Notizen zum Flugtag")).toBeVisible();

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
    await fakeDocument(page, [upload(1, "Notizen.txt", "text/plain"), draft]);
    await page.goto(`/documents/${DOCUMENT_ID}?pseudo`);
    await expect(page.getByRole("table")).toBeVisible();
    expect(await textOverflows(page)).toEqual([]);
  });
}
