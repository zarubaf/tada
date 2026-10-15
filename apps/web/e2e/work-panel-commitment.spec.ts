import { expect, type Page, test } from "@playwright/test";
import { axeBuilder, fontsLoaded, setTheme, themes, viewports } from "./fixtures";
import { commitments, event, fakeWork, fulfill } from "./workFixtures";

// A commitment in the side panel (ADR 0082): the text, the properties, „Verbindlich machen“ with its
// reason, and the create form with the search for the promisor. The unit tests hold the rules;
// these checks hold what only a browser shows: the look of the panel beside the one of an action,
// the dialog over it, the 44 px targets on touch, and the screenshots.
const work = `/events/${event?.id}/work`;

// React Aria speaks through a hidden element after a choice (an option, a menu). While it speaks, the
// element is a role=img that names itself by text that is gone a moment later; it is the library's
// own and no part of the page, so the check leaves it out.
const axe = (page: Page) => axeBuilder(page).exclude("[data-live-announcer]").analyze();

// COM-001 with everything the panel can show: a condition, the next statuses and a source.
const commitment = {
  ...commitments[0],
  condition: "Die Bestellung ist unterschrieben und die Anzahlung ist eingegangen.",
  can_make_firm: true,
  next_statuses: ["fulfilled", "broken", "withdrawn"],
  evidence: [
    {
      proposal_id: "0199b8e0-0000-7000-8000-000000000501",
      record_version: 1,
      source_version_id: "0199b8e0-0000-7000-8000-000000000601",
      captured_at: "2030-03-01T10:00:00Z",
      start_offset: 0,
      end_offset: 40,
      quote: "Wir liefern den Generator am Freitag, sobald die Bestellung unterschrieben ist.",
      redacted: false,
    },
  ],
};

async function open(page: Page, search: string) {
  await fakeWork(page, { commitments: [commitment] });
  await fulfill(page, "**/api/v1/persons*", { items: [] });
  await fulfill(page, "**/api/v1/institutions*", {
    items: [
      {
        id: "0199b8e0-0000-7000-8000-0000000000d1",
        local_id: "INS-001",
        name: "Generatorenvermietung Testwil-Oberdorf AG",
        version: 1,
      },
    ],
  });
  // The baselines must not depend on the real date: it decides what is overdue.
  await page.clock.setFixedTime(new Date("2030-05-08T10:00:00Z"));
  await page.goto(`${work}${search}`);
  await expect(page.getByRole("heading", { level: 1, name: "Arbeit" })).toBeVisible();
}

async function openStep(page: Page) {
  const panel = page.getByRole("complementary", { name: /^COM-001/ });
  await panel.getByRole("button", { name: "Aktionen" }).click();
  await page.getByRole("menuitem", { name: "Verbindlich machen" }).click();
  const dialog = page.getByRole("dialog", { name: "COM-001 verbindlich machen" });
  await expect(dialog).toBeVisible();
  return dialog;
}

for (const viewport of viewports) {
  const narrow = viewport.width < 640;
  for (const theme of themes) {
    const name = `${theme}, ${viewport.name} px`;

    test(`open on a commitment, ${name}: no axe violation, screenshot`, async ({ page }) => {
      await page.setViewportSize(viewport);
      await open(page, "?view=table#COM-001");
      await setTheme(page, theme);
      const panel = page.getByRole("complementary", { name: /^COM-001/ });
      await expect(panel).toBeVisible();
      await expect(panel.getByRole("heading", { level: 3, name: "Quellen" })).toBeVisible();
      await expect(panel.getByRole("button", { name: "Aktionen" })).toBeVisible();
      // The panel slides in; measure it when it has stopped.
      await expect
        .poll(async () => (await panel.boundingBox())?.x)
        .toBe(narrow ? 0 : viewport.width - 480);
      expect((await axe(page)).violations).toEqual([]);
      await fontsLoaded(page);
      await expect(page).toHaveScreenshot(`commitment-open-${theme}-${viewport.name}.png`);
    });

    test(`the step of Verbindlich machen, ${name}: no axe violation, screenshot`, async ({
      page,
    }) => {
      await page.setViewportSize(viewport);
      await open(page, "#COM-001");
      await setTheme(page, theme);
      const dialog = await openStep(page);
      await expect(dialog.getByRole("textbox", { name: "Grund (Pflichtfeld)" })).toBeFocused();
      await expect(dialog.getByText(/Bedingung: Die Bestellung/)).toBeVisible();
      expect((await axe(page)).violations).toEqual([]);
      await fontsLoaded(page);
      await expect(page).toHaveScreenshot(`commitment-firm-${theme}-${viewport.name}.png`);
    });

    test(`create state, ${name}: no axe violation, screenshot`, async ({ page }) => {
      await page.setViewportSize(viewport);
      await open(page, "?new=commitment");
      await setTheme(page, theme);
      const panel = page.getByRole("complementary", { name: "Neue Zusage" });
      await expect(panel.getByRole("textbox", { name: /^Zusage/ })).toBeVisible();
      await expect(panel.getByRole("combobox", { name: /^Zugesagt von/ })).toBeVisible();
      expect((await axe(page)).violations).toEqual([]);
      await fontsLoaded(page);
      await expect(page).toHaveScreenshot(`commitment-create-${theme}-${viewport.name}.png`);
    });
  }
}

test("make firm shows the new status in the panel and the card, and the dialog does not close the panel on Escape", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await open(page, "");
  const firm: Record<string, unknown>[] = [];
  await page.route("**/api/v1/events/*/commitments/*/firm", async (route) => {
    firm.push(route.request().postDataJSON());
    return route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        ...commitment,
        status: "firm",
        firm_reason: "Bestellung unterschrieben",
        can_make_firm: false,
        version: 2,
      }),
    });
  });
  await page.getByRole("link", { name: /COM-001/ }).click();
  const panel = page.getByRole("complementary", { name: /^COM-001/ });
  const dialog = await openStep(page);

  // Escape closes the dialog and not the panel; focus is back on the menu button.
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await expect(panel).toBeVisible();
  await expect(panel.getByRole("button", { name: "Aktionen" })).toBeFocused();

  const again = await openStep(page);
  await again.getByRole("button", { name: "Verbindlich machen" }).click();
  await expect(again.getByText(/Geben Sie einen Grund an/)).toBeVisible();
  await again.getByRole("textbox", { name: /^Grund/ }).fill("Bestellung unterschrieben");
  await again.getByRole("button", { name: "Verbindlich machen" }).click();

  await expect(panel.getByRole("button", { name: "Status: verbindlich" })).toBeVisible();
  await expect(panel.getByText("Bestellung unterschrieben", { exact: true })).toBeVisible();
  await expect(panel.getByRole("button", { name: "Aktionen" })).toHaveCount(0);
  // The menu leaves with the state; focus goes to the title, which is a button for a member who may rename.
  await expect(panel.getByRole("heading", { level: 2 }).getByRole("button")).toBeFocused();
  expect(firm).toEqual([{ reason: "Bestellung unterschrieben", expected_version: 1 }]);
  expect((await axe(page)).violations).toEqual([]);
});

test("the promisor is searched on the server and sent with the new commitment", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await open(page, "?new=commitment");
  const posted: Record<string, unknown>[] = [];
  await page.route("**/api/v1/events/*/commitments", async (route) => {
    if (route.request().method() !== "POST") {
      return route.fallback();
    }
    const body = route.request().postDataJSON();
    posted.push(body);
    return route.fulfill({
      status: 201,
      contentType: "application/json",
      body: JSON.stringify({ ...commitment, id: body.id, local_id: "COM-002", text: body.text }),
    });
  });
  const panel = page.getByRole("complementary", { name: "Neue Zusage" });
  await panel.getByRole("textbox", { name: /^Zusage/ }).fill("Wir liefern zwei Festzelte");
  await panel.getByRole("combobox", { name: /^Zugesagt von/ }).fill("Generator");
  await page.getByRole("option", { name: /Generatorenvermietung Testwil-Oberdorf AG/ }).click();
  await panel.getByRole("button", { name: "Erstellen" }).click();

  await expect(page.getByRole("complementary", { name: /^COM-002/ })).toBeVisible();
  expect(posted).toEqual([
    {
      id: expect.any(String),
      text: "Wir liefern zwei Festzelte",
      promisor: { kind: "institution", id: "0199b8e0-0000-7000-8000-0000000000d1" },
      owner_user_id: expect.any(String),
    },
  ]);
});

test.describe("on a touch screen", () => {
  test.use({ viewport: { width: 375, height: 812 }, hasTouch: true, isMobile: true });

  test("every control of the sheet and of the dialog is 44 px high", async ({ page }) => {
    await open(page, "#COM-001");
    const panel = page.getByRole("complementary", { name: /^COM-001/ });
    await expect(panel).toBeVisible();
    const heights = (selector: ReturnType<Page["locator"]>) =>
      selector.evaluateAll((elements) =>
        elements.map((element) => ({
          name: element.getAttribute("aria-label") ?? element.textContent?.trim().slice(0, 30),
          height: element.getBoundingClientRect().height,
        })),
      );
    const controls = await heights(panel.locator("button, input, textarea, a[href]"));
    expect(controls.length).toBeGreaterThan(6);
    for (const { name, height } of controls) {
      expect([name, height >= 44]).toEqual([name, true]);
    }
    const dialog = await openStep(page);
    for (const { name, height } of await heights(dialog.locator("button, textarea"))) {
      expect([name, height >= 44]).toEqual([name, true]);
    }
  });
});
