import type { Page } from "@playwright/test";

/** Invented events with long German names and umlauts (doc/design/principles.md). */
export const events = [
  {
    id: "0199b8e0-0000-7000-8000-000000000001",
    key: "FLY28",
    name: "Fly-in Musterhausen mit Veranstaltungsbewilligungsverfahren",
    time_zone: "Europe/Zurich",
    version: 1,
    created_at: "2028-03-01T13:12:00Z",
  },
  {
    id: "0199b8e0-0000-7000-8000-000000000002",
    key: "TEST30",
    name: "Tag der offenen Tür Testwil",
    time_zone: "Europe/Zurich",
    version: 1,
    created_at: "2030-05-18T08:00:00Z",
  },
];

const membership = {
  organization_id: "0199b8e0-0000-7000-8000-0000000000a1",
  name: "Fliegergruppe Testwil",
  role: "member",
};

/** An invented signed-in member with one organization. */
export const sessionInfo = {
  user_id: "0199b8e0-0000-7000-8000-0000000000b1",
  display_name: "Anna Muster",
  organization: membership,
  memberships: [membership],
};

/** A session whose organization role is `role`. */
export function sessionWithRole(role: string) {
  const own = { ...membership, role };
  return { ...sessionInfo, organization: own, memberships: [own] };
}

/**
 * The fake API of the event screens: the answer of `GET /api/v1/events/{id}`. The event has an
 * empty profile and no fields; `fakeProfile` after it sets a profile.
 */
export async function fakeEvent(page: Page, body: unknown, status = 200): Promise<void> {
  await fakeProfile(page, { facts: [], proposals: [], open_questions: [] }, []);
  await page.route("**/api/v1/events/*", (route) =>
    route.fulfill({
      status,
      contentType: status < 400 ? "application/json" : "application/problem+json",
      body: JSON.stringify(body),
    }),
  );
}

/** The fake API of the event overview: the answers of `GET .../profile` and `GET .../fields`. */
export async function fakeProfile(page: Page, profile: unknown, fields: unknown[]): Promise<void> {
  await page.route("**/api/v1/events/*/profile", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(profile) }),
  );
  await page.route("**/api/v1/events/*/fields", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ items: fields }),
    }),
  );
}

/** The fake API of the browser checks: the answer of `GET /api/v1/session`. */
export async function fakeSession(page: Page, info: unknown = sessionInfo): Promise<void> {
  await page.route("**/api/v1/session", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(info) }),
  );
}

/** The fake API of the browser checks: the answer of `GET /api/v1/events`. */
export async function fakeEvents(page: Page, status: number, body: unknown): Promise<void> {
  await page.route("**/api/v1/events*", (route) =>
    route.fulfill({
      status,
      contentType: status < 400 ? "application/json" : "application/problem+json",
      body: JSON.stringify(body),
    }),
  );
}

export const unavailable = {
  type: "https://github.com/zarubaf/tada/blob/main/doc/problems.md#unavailable",
  code: "unavailable",
  title: "A dependency is unavailable. The client can retry.",
  status: 503,
  instance: "urn:uuid:01a1118e-3359-73dd-a500-feed65806a9d",
  request_id: "01a1118e-3359-73dd-a500-feed65806a9d",
};

/** A problem body for the fake API. */
export function problemBody(code: string, status: number) {
  return { ...unavailable, code, status };
}

// The fake API of the sign-in pages.

/** A client without a session: the answer of `GET /api/v1/session`. */
export async function fakeSignedOut(page: Page): Promise<void> {
  await page.route("**/api/v1/session", (route) =>
    route.fulfill({
      status: 401,
      contentType: "application/problem+json",
      body: JSON.stringify(problemBody("unauthenticated", 401)),
    }),
  );
}

/** Answers a POST to `path` with the given status and JSON body. */
export async function fakePost(
  page: Page,
  path: string,
  status: number,
  body?: unknown,
): Promise<void> {
  await page.route(`**${path}`, (route) =>
    route.fulfill({
      status,
      contentType: status < 400 ? "application/json" : "application/problem+json",
      body: body === undefined ? "" : JSON.stringify(body),
    }),
  );
}

export const invitationPreview = { organization_name: "Fliegergruppe Testwil", role: "member" };

export const viewports = [
  { name: "375", width: 375, height: 812 },
  { name: "1440", width: 1440, height: 900 },
] as const;

export const themes = ["light", "dark"] as const;

/** Sets the theme on the root element, as the settings will do (ADR 0018). */
export async function setTheme(page: Page, theme: (typeof themes)[number]): Promise<void> {
  await page.emulateMedia({ colorScheme: theme });
  await page.evaluate((value) => document.documentElement.setAttribute("data-theme", value), theme);
}

/**
 * Loads both web fonts before a screenshot. `document.fonts.ready` alone is not enough: a font whose
 * text appears late starts to load after `ready` resolved (`font-display: swap`).
 */
export async function fontsLoaded(page: Page): Promise<void> {
  await page.evaluate(async () => {
    await Promise.all([
      document.fonts.load('400 1rem "Mona Sans Variable"', "Aä"),
      document.fonts.load('600 1rem "Mona Sans Variable"', "Aä"),
      document.fonts.load('400 1rem "JetBrains Mono Variable"', "Aä"),
    ]);
    await document.fonts.ready;
  });
}

/**
 * The elements whose text overflows its box, for the pseudo-locale check (ADR 0024). A scroll
 * container is not an overflow, but the page itself must not scroll sideways.
 */
export async function textOverflows(page: Page): Promise<string[]> {
  return page.evaluate(() => {
    const found: string[] = [];
    for (const element of document.querySelectorAll<HTMLElement>("body *")) {
      const style = getComputedStyle(element);
      const scrolls = ["auto", "scroll"].includes(style.overflowX);
      // Visually hidden text, for example a table caption, has no box to overflow.
      const visuallyHidden = style.clipPath !== "none";
      if (
        scrolls ||
        visuallyHidden ||
        element.title ||
        element.childElementCount > 0 ||
        !element.textContent?.trim()
      ) {
        continue;
      }
      if (element.scrollWidth > element.clientWidth + 1 && element.clientWidth > 0) {
        found.push(`${element.tagName}: ${element.textContent.trim().slice(0, 40)}`);
      }
    }
    if (document.documentElement.scrollWidth > window.innerWidth) {
      found.push("the page scrolls horizontally");
    }
    return found;
  });
}
