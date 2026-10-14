import AxeBuilder from "@axe-core/playwright";
import { expect, type Locator, type Page, test } from "@playwright/test";
import { fontsLoaded, problemBody, setTheme, textOverflows, themes, viewports } from "./fixtures";
import { event, fakeWork, fulfill } from "./workFixtures";

const axe = (page: Page) =>
  new AxeBuilder({ page }).withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"]).analyze();

/** Waits for the board: the columns (wide) or the status picker (narrow). */
async function boardShown(page: Page, narrow: boolean) {
  if (narrow) {
    await expect(page.getByRole("button", { name: /Offen \(\d+\)/ })).toBeVisible();
  } else {
    await expect(page.getByRole("heading", { level: 2, name: /^Offen/ })).toBeVisible();
  }
  await expect(page.getByRole("status", { name: "Arbeit wird geladen" })).toHaveCount(0);
}

const work = `/events/${event?.id}/work`;

for (const viewport of viewports) {
  for (const theme of themes) {
    test(`board, ${theme}, ${viewport.name} px: no axe violation, screenshot`, async ({ page }) => {
      await page.setViewportSize(viewport);
      await fakeWork(page);
      await page.goto(work);
      await setTheme(page, theme);
      await expect(page.getByRole("heading", { level: 1, name: "Arbeit" })).toBeVisible();
      await boardShown(page, viewport.width < 640);

      expect((await axe(page)).violations).toEqual([]);
      await fontsLoaded(page);
      await expect(page).toHaveScreenshot(`work-board-${theme}-${viewport.name}.png`, {
        fullPage: true,
      });
    });
  }
}

for (const theme of themes) {
  test(`collapsed column opened, ${theme}: no axe violation`, async ({ page }) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    await fakeWork(page);
    await page.goto(work);
    await setTheme(page, theme);
    await page.getByRole("button", { name: /^Abgebrochen/ }).click();
    await expect(page.getByRole("heading", { level: 2, name: /^Abgebrochen/ })).toBeVisible();
    expect((await axe(page)).violations).toEqual([]);
  });

  test(`filter sheet on a phone, ${theme}: no axe violation, screenshot`, async ({ page }) => {
    await page.setViewportSize({ width: 375, height: 812 });
    await fakeWork(page);
    await page.goto(work);
    await setTheme(page, theme);
    await boardShown(page, true);
    await page.getByRole("button", { name: "Filter" }).click();
    await expect(page.getByRole("dialog")).toBeVisible();
    expect((await axe(page)).violations).toEqual([]);
    await fontsLoaded(page);
    await expect(page).toHaveScreenshot(`work-board-filter-sheet-${theme}-375.png`);
  });

  test(`a failed kind shows its banner, ${theme}: no axe violation`, async ({ page }) => {
    await fakeWork(page);
    await fulfill(page, "**/api/v1/events/*/risks*", problemBody("unavailable", 503), 503);
    await page.goto(work);
    await setTheme(page, theme);
    // A hidden polite region repeats the text for screen readers: take the banner.
    await expect(
      page.locator("strong", { hasText: "Risiken konnten nicht geladen werden." }),
    ).toBeVisible();
    await expect(page.getByText("ACT-001", { exact: true })).toBeVisible();
    expect((await axe(page)).violations).toEqual([]);
  });

  test(`empty event, ${theme}: no axe violation`, async ({ page }) => {
    await fakeWork(page, {
      actions: [],
      commitments: [],
      milestones: [],
      decisions: [],
      risks: [],
    });
    await page.goto(work);
    await setTheme(page, theme);
    await expect(page.getByText("Noch keine Arbeit in diesem Anlass")).toBeVisible();
    expect((await axe(page)).violations).toEqual([]);
  });
}

test("a URL opens the same filtered view after a reload", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await fakeWork(page);
  await page.goto(`${work}?type=action&q=Flyer`);
  await expect(page.getByRole("link", { name: /ACT-003/ })).toBeVisible();
  await expect(page.getByRole("link", { name: /ACT-001/ })).toHaveCount(0);
  await page.reload();
  await expect(page.getByRole("link", { name: /ACT-003/ })).toBeVisible();
  // The search sits in the sheet of the „Filter“ button, which says how many filters are set.
  await page.getByRole("button", { name: "Filter, 1 gesetzt" }).click();
  await expect(page.getByRole("searchbox", { name: "Arbeit durchsuchen" })).toHaveValue("Flyer");
  await expect(page.getByRole("radio", { name: /^Aufgaben/, checked: true })).toBeVisible();
});

test("a card is one link, and its address opens the panel at the record", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await fakeWork(page);
  await page.goto(work);
  const link = page.getByRole("link", { name: /ACT-001/ });
  const href = `/events/${event?.id}/work#ACT-001`;
  await expect(link).toHaveAttribute("href", href);
  await expect(link).toHaveAccessibleName(/^ACT-001 Generatorenvermietung/);
  // The other kinds keep their old pages until their panels exist.
  await expect(page.getByRole("link", { name: /COM-001/ })).toHaveAttribute(
    "href",
    `/events/${event?.id}/commitments#COM-001`,
  );
  // The address works on its own, for a shared link or a reload: it opens the record in the panel.
  await page.goto(href);
  await expect(page.getByRole("complementary", { name: /^ACT-001/ })).toBeVisible();
});

// ADR 0022: the targets are 44 px with a coarse pointer, on the board, the filter sheet and the
// controls of the phone.
for (const width of [320, 375]) {
  test.describe(`at ${width} px, touch`, () => {
    test.use({ hasTouch: true, isMobile: true });

    test("every control of the work page is 44 px high", async ({ page }) => {
      await page.setViewportSize({ width, height: 800 });
      await fakeWork(page);
      await page.goto(`${work}?q=Flyer`);
      // The search matches an action in progress, so the list opens on that status.
      await expect(page.getByRole("link", { name: /ACT-003/ })).toBeVisible();
      // The input of a checkbox is hidden; its label is the target, and work-bulk.spec.ts measures it.
      const targets = async (scope: Locator) => {
        const tall: { name: string | null; height: number | undefined }[] = [];
        for (const target of await scope.locator("button, a, input:not([type=checkbox])").all()) {
          if (await target.isVisible()) {
            const height = (await target.boundingBox())?.height;
            if (height === undefined || height < 44) {
              tall.push({ name: await target.getAttribute("aria-label"), height });
            }
          }
        }
        return tall;
      };
      // The search is in the sheet on a phone; its clear button shows because of `q`.
      expect(await targets(page.getByRole("main"))).toEqual([]);
      await page.getByRole("button", { name: /^Filter/ }).click();
      const dialog = page.getByRole("dialog");
      await expect(dialog.getByRole("button", { name: "Suche leeren" })).toBeVisible();
      expect(await targets(dialog)).toEqual([]);
    });
  });
}

// ADR 0024: no text overflows with the 40 % longer pseudo-locale, and a phone does not scroll
// sideways (ADR 0023).
for (const viewport of viewports) {
  test(`pseudo-locale, ${viewport.name} px: no text overflows`, async ({ page }) => {
    await page.setViewportSize(viewport);
    await fakeWork(page);
    await page.goto(`${work}?pseudo`);
    // The pseudo-locale changes the words, so the first card is the sign that the board shows.
    await expect(page.locator("a[href$='#ACT-001']")).toBeVisible();
    expect(await textOverflows(page)).toEqual([]);
  });
}

test("at 320 px the board does not scroll sideways", async ({ page }) => {
  await page.setViewportSize({ width: 320, height: 800 });
  await fakeWork(page);
  await page.goto(work);
  await boardShown(page, true);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(
    true,
  );
});
