import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";
import {
  fakePost,
  fakeSession,
  fontsLoaded,
  problemBody,
  sessionWithRole,
  setTheme,
  textOverflows,
  themes,
  viewports,
} from "./fixtures";

// Invented values that follow `PmPolicy`, `AiStatus` and `DigestList` of contracts/openapi.json.
const policy = {
  local_time: "08:30:00",
  time_zone: "Europe/Zurich",
  weekdays: 31,
  quiet_start: "20:00:00",
  quiet_end: "08:00:00",
  cooldown_hours: 20,
  follow_up_working_days: 2,
  look_ahead_working_days: 2,
  paused: false,
  ai_enabled: true,
  version: 3,
};

const ai = { enabled: true, configured: false, cap_chf: "25.00", used_chf: "0.00", version: 1 };

const digests = [
  {
    id: "0199b8e0-0000-7000-8000-0000000000d1",
    user_display_name: "Bernadette Müller-Lüdenscheidt",
    created_at: "2030-05-17T06:30:00Z",
    sent_at: null,
    status: "unknown",
    item_count: 4,
  },
  {
    id: "0199b8e0-0000-7000-8000-0000000000d2",
    user_display_name: "Anna Muster",
    created_at: "2030-05-17T06:30:00Z",
    sent_at: "2030-05-17T06:30:12Z",
    status: "sent",
    item_count: 2,
  },
  {
    id: "0199b8e0-0000-7000-8000-0000000000d3",
    user_display_name: "Jürg Zbinden",
    created_at: "2030-05-16T06:30:00Z",
    sent_at: null,
    status: "failed",
    item_count: 1,
  },
  {
    id: "0199b8e0-0000-7000-8000-0000000000d4",
    user_display_name: "Cäcilia Probst",
    created_at: "2030-05-16T06:30:00Z",
    sent_at: null,
    status: "canceled",
    item_count: 0,
  },
];

function fulfill(status: number, body: unknown) {
  return {
    status,
    contentType: status < 400 ? "application/json" : "application/problem+json",
    body: JSON.stringify(body),
  };
}

/** The fake API of the AI-PM page: the policy, the AI state and the last digests. */
async function fakePm(page: Page, policyBody: unknown = policy, status = 200): Promise<void> {
  await page.route("**/api/v1/organization/pm-policy", (route) =>
    route.fulfill(fulfill(status, policyBody)),
  );
  await page.route("**/api/v1/organization/ai", (route) =>
    route.fulfill(fulfill(status, status === 200 ? ai : policyBody)),
  );
  await page.route("**/api/v1/organization/digests", (route) =>
    route.fulfill(fulfill(200, { items: digests })),
  );
}

for (const viewport of viewports) {
  for (const theme of themes) {
    test(`ai-pm, ${theme}, ${viewport.name} px: no axe violation, screenshot`, async ({ page }) => {
      await page.setViewportSize(viewport);
      await fakeSession(page, sessionWithRole("owner"));
      await fakePm(page);
      await page.goto("/settings?tab=ai-pm");
      await setTheme(page, theme);
      await expect(page.getByRole("heading", { level: 2, name: "AI-PM" })).toBeVisible();
      await expect(page.getByRole("table", { name: "Letzte Erinnerungen" })).toBeVisible();

      const results = await new AxeBuilder({ page })
        .withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"])
        .analyze();
      expect(results.violations).toEqual([]);

      await fontsLoaded(page);
      await expect(page).toHaveScreenshot(`ai-pm-${theme}-${viewport.name}.png`, {
        fullPage: true,
      });
    });

    test(`ai-pm, member, ${theme}, ${viewport.name} px: no access, no axe violation, screenshot`, async ({
      page,
    }) => {
      await page.setViewportSize(viewport);
      await fakeSession(page);
      // A member asks for nothing of the AI-PM: the client knows the role.
      const asked: string[] = [];
      page.on("request", (request) => {
        if (/\/api\/v1\/organization\/(pm-policy|ai|digests)/.test(request.url())) {
          asked.push(request.url());
        }
      });
      await page.goto("/settings?tab=ai-pm");
      await setTheme(page, theme);

      await expect(page.getByText("Sie haben keinen Zugriff auf diesen Bereich.")).toBeVisible();
      expect(asked).toEqual([]);
      await expect(page.getByRole("link", { name: "AI-PM" })).toHaveCount(0);
      await expect(page.getByRole("textbox")).toHaveCount(0);

      const results = await new AxeBuilder({ page })
        .withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"])
        .analyze();
      expect(results.violations).toEqual([]);

      await fontsLoaded(page);
      await expect(page).toHaveScreenshot(`ai-pm-member-${theme}-${viewport.name}.png`, {
        fullPage: true,
      });
    });

    test(`ai-pm, paused, ${theme}, ${viewport.name} px: the banner, no axe violation, screenshot`, async ({
      page,
    }) => {
      await page.setViewportSize(viewport);
      await fakeSession(page, sessionWithRole("owner"));
      await fakePm(page, { ...policy, paused: true });
      await page.goto("/settings?tab=ai-pm");
      await setTheme(page, theme);

      await expect(page.getByText("Der AI-PM ist pausiert.")).toBeVisible();
      await expect(page.getByRole("switch", { name: "Pause für alle Nachrichten" })).toBeChecked();
      await expect(page.getByRole("table", { name: "Letzte Erinnerungen" })).toBeVisible();

      const results = await new AxeBuilder({ page })
        .withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"])
        .analyze();
      expect(results.violations).toEqual([]);

      await fontsLoaded(page);
      // The top of the page is enough: the banner and the switch.
      await expect(page).toHaveScreenshot(`ai-pm-paused-${theme}-${viewport.name}.png`);
    });
  }
}

test("an admin finds the page in the settings", async ({ page }) => {
  await fakeSession(page, sessionWithRole("admin"));
  await fakePm(page);
  await page.goto("/settings?tab=organization");

  await page.getByRole("tab", { name: "AI-PM" }).click();
  await expect(page).toHaveURL(/\/settings\?tab=ai-pm$/);
  await expect(page.getByRole("heading", { level: 2, name: "AI-PM" })).toBeVisible();
});

test("the unknown outcome of a digest is visible with its meaning", async ({ page }) => {
  await fakeSession(page, sessionWithRole("owner"));
  await fakePm(page);
  await page.goto("/settings?tab=ai-pm");

  const row = page.getByRole("row", { name: /Bernadette Müller-Lüdenscheidt/ });
  await expect(row).toContainText("unbekannt");
  await expect(row).toContainText("Nicht gesendet");
  await expect(page.getByText(/Unbekannt heisst/)).toBeVisible();
  await expect(page.getByText("nicht eingerichtet")).toBeVisible();
});

test("a version conflict shows the new values of the policy", async ({ page }) => {
  await fakeSession(page, sessionWithRole("owner"));
  await fakePm(page);
  await fakePost(
    page,
    "/api/v1/organization/pm-policy/set",
    409,
    problemBody("record-version-conflict", 409),
  );
  await page.goto("/settings?tab=ai-pm");

  const cooldown = page.getByRole("textbox", { name: "Abklingzeit (Stunden)" });
  await cooldown.fill("30");
  await page.route("**/api/v1/organization/pm-policy", (route) =>
    route.fulfill(fulfill(200, { ...policy, cooldown_hours: 48, version: 4 })),
  );
  await page.getByRole("button", { name: "Einstellungen speichern" }).click();

  await expect(page.getByText(/Jemand hat die Einstellungen inzwischen geändert/)).toBeVisible();
  await expect(cooldown).toHaveValue("48");
});

// ADR 0024: no text overflows with the 40 % longer pseudo-locale.
for (const viewport of viewports) {
  test(`ai-pm, pseudo-locale, ${viewport.name} px: no text overflows`, async ({ page }) => {
    await page.setViewportSize(viewport);
    await fakeSession(page, sessionWithRole("owner"));
    await fakePm(page);
    await page.goto("/settings?tab=ai-pm&pseudo");
    await expect(page.getByRole("table")).toBeVisible();
    await expect(page.locator("[role=status][aria-label]")).toHaveCount(0);

    expect(await textOverflows(page)).toEqual([]);
  });
}

/**
 * The elements of `main` whose right edge is past the content edge of `main`: an overflow into
 * the page gutter. A visually hidden element, for example the input of a switch, has no edge.
 */
async function gutterOverflows(page: Page): Promise<string[]> {
  return page.evaluate(() => {
    const main = document.querySelector("main");
    if (!main) {
      return ["no main"];
    }
    const edge =
      main.getBoundingClientRect().right - Number.parseFloat(getComputedStyle(main).paddingRight);
    const found: string[] = [];
    for (const element of main.querySelectorAll<HTMLElement>("*")) {
      const style = getComputedStyle(element);
      const rect = element.getBoundingClientRect();
      if (style.clipPath !== "none" || style.opacity === "0" || rect.width <= 1) {
        continue;
      }
      if (rect.right > edge + 0.5) {
        found.push(`${element.tagName}.${element.className}: ${rect.right - edge} px`);
      }
    }
    if (document.documentElement.scrollWidth > window.innerWidth) {
      found.push("the page scrolls horizontally");
    }
    return found;
  });
}

// WCAG reflow: the quiet hours and the rest of the form stay inside the page gutter.
for (const width of [320, 375]) {
  test(`ai-pm, ${width} px: nothing overflows into the gutter`, async ({ page }) => {
    await page.setViewportSize({ width, height: 800 });
    await fakeSession(page, sessionWithRole("owner"));
    await fakePm(page);
    await page.goto("/settings?tab=ai-pm");
    await expect(page.getByRole("textbox", { name: "Ruhezeit bis" })).toBeVisible();
    await expect(page.getByRole("table")).toBeVisible();

    expect(await gutterOverflows(page)).toEqual([]);
  });
}
