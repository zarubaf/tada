import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";
import {
  fakePost,
  fakeSession,
  fakeSignedOut,
  fontsLoaded,
  invitationPreview,
  problemBody,
  sessionInfo,
  setTheme,
  textOverflows,
  themes,
  viewports,
} from "./fixtures";

// The pages that a member without a session can open, each in a state with all its content.
const publicPages = [
  {
    name: "sign-in",
    path: "/sign-in",
    hash: "",
    heading: "Anmelden",
    ready: "E-Mail-Adresse (Pflichtfeld)",
    prepare: async (_page: Page) => {},
  },
  {
    name: "magic link",
    path: "/sign-in/link",
    hash: "#token=invented-token",
    heading: "Anmelden",
    ready: "Mit dem Klick melden Sie sich in tada an.",
    prepare: async (_page: Page) => {},
  },
  {
    name: "magic link, invalid",
    path: "/sign-in/link",
    hash: "",
    heading: "Anmelden",
    ready: "Dieser Link ist ungültig oder abgelaufen.",
    prepare: async (_page: Page) => {},
  },
  {
    name: "choose organization",
    path: "/choose-organization",
    hash: "",
    heading: "Organisation wählen",
    ready: "Segelflugclub Musterhausen",
    prepare: (page: Page) =>
      fakeSession(page, {
        user_id: sessionInfo.user_id,
        display_name: sessionInfo.display_name,
        memberships: [
          sessionInfo.organization,
          { ...sessionInfo.organization, organization_id: "x", name: "Segelflugclub Musterhausen" },
        ],
      }),
  },
  {
    name: "invitation",
    path: "/invitation",
    hash: "#token=invented-token",
    heading: "Einladung",
    ready: "Sie sind eingeladen: Organisation Fliegergruppe Testwil, Rolle Mitglied.",
    prepare: (page: Page) => fakePost(page, "/api/v1/invitations/preview", 200, invitationPreview),
  },
  {
    name: "invitation, invalid",
    path: "/invitation",
    hash: "#token=invented-token",
    heading: "Einladung",
    ready: "Diese Einladung ist ungültig oder abgelaufen.",
    prepare: (page: Page) =>
      fakePost(page, "/api/v1/invitations/preview", 401, problemBody("unauthenticated", 401)),
  },
];

for (const viewport of viewports) {
  for (const theme of themes) {
    for (const entry of publicPages) {
      test(`${entry.name}, ${theme}, ${viewport.name} px: no axe violation`, async ({ page }) => {
        await page.setViewportSize(viewport);
        await fakeSignedOut(page);
        await entry.prepare(page);
        await page.goto(`${entry.path}${entry.hash}`);
        await setTheme(page, theme);
        await expect(page.getByRole("heading", { name: entry.heading })).toBeVisible();
        await expect(page.getByText(entry.ready)).toBeVisible();
        await expect(page.getByRole("status").filter({ hasText: /\S/ })).toHaveCount(0);

        const results = await new AxeBuilder({ page })
          .withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"])
          .analyze();
        expect(results.violations).toEqual([]);
      });
    }

    test(`sign-in, ${theme}, ${viewport.name} px: screenshot`, async ({ page }) => {
      await page.setViewportSize(viewport);
      await fakeSignedOut(page);
      await page.goto("/sign-in");
      await setTheme(page, theme);
      await expect(page.getByLabel("E-Mail-Adresse")).toBeVisible();
      await fontsLoaded(page);
      await expect(page).toHaveScreenshot(`sign-in-${theme}-${viewport.name}.png`, {
        fullPage: true,
      });
    });
  }
}

test("the magic link page keeps the token out of the address and signs in only on the click", async ({
  page,
}) => {
  await fakeSignedOut(page);
  const redeemed: string[] = [];
  await page.route("**/api/v1/sign-in/magic-link", (route) => {
    redeemed.push(route.request().postData() ?? "");
    return route.fulfill({
      status: 401,
      contentType: "application/problem+json",
      body: JSON.stringify(problemBody("unauthenticated", 401)),
    });
  });
  await page.goto("/sign-in/link#token=invented-token");
  await expect(page.getByRole("button", { name: "Anmelden" })).toBeVisible();
  expect(new URL(page.url()).hash).toBe("");
  expect(redeemed).toEqual([]);

  await page.getByRole("button", { name: "Anmelden" }).click();
  // The message takes focus, because the button left with the final failure.
  await expect(page.getByRole("alert").filter({ hasText: "ungültig" })).toBeFocused();
  expect(redeemed).toEqual(['{"token":"invented-token"}']);
  await page.getByRole("link", { name: "Zur Anmeldung" }).click();
  await expect(page).toHaveURL(/\/sign-in$/);
});

test("the sign-in page works with the keyboard only", async ({ page }) => {
  await fakeSignedOut(page);
  await fakePost(page, "/api/v1/sign-in/requests", 202);
  await page.goto("/sign-in");
  await expect(page.getByLabel("E-Mail-Adresse")).toBeVisible();

  await page.keyboard.press("Tab");
  await expect(page.getByRole("link", { name: "Zum Inhalt springen" })).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(page.getByLabel("E-Mail-Adresse")).toBeFocused();
  await page.keyboard.type("anna.muster@example.org");
  await page.keyboard.press("Tab");
  await expect(page.getByRole("button", { name: "Anmeldelink senden" })).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(
    page.getByText(
      "Wenn die Adresse bekannt ist, ist eine E-Mail mit einem Anmeldelink unterwegs oder schon in Ihrem Postfach. Verwenden Sie den Link in der neuesten E-Mail.",
    ),
  ).toBeVisible();
});

test("the sign-in page shows the message of the rate limit", async ({ page }) => {
  await fakeSignedOut(page);
  await fakePost(page, "/api/v1/sign-in/requests", 429, problemBody("rate-limited", 429));
  await page.goto("/sign-in");
  await page.getByLabel("E-Mail-Adresse").fill("anna.muster@example.org");
  await page.getByRole("button", { name: "Anmeldelink senden" }).click();
  await expect(page.getByRole("alert")).toContainText("Zu viele Anfragen.");
});

// ADR 0024: German text can be 40 % longer than the source.
for (const viewport of viewports) {
  for (const entry of publicPages) {
    test(`${entry.name}, pseudo-locale, ${viewport.name} px: no text overflows`, async ({
      page,
    }) => {
      await page.setViewportSize(viewport);
      await fakeSignedOut(page);
      await entry.prepare(page);
      await page.goto(`${entry.path}?pseudo${entry.hash}`);
      await expect(page.getByRole("heading", { name: /^\[/ })).toBeVisible();
      await expect(page.getByRole("status").filter({ hasText: /\S/ })).toHaveCount(0);
      expect(await textOverflows(page)).toEqual([]);
    });
  }
}
