import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";
import {
  events,
  fakeSession,
  fontsLoaded,
  sessionInfo,
  setTheme,
  textOverflows,
  themes,
  viewports,
} from "./fixtures";

const event = events[0];
const ME = sessionInfo.user_id;

// Invented records. The due dates are far from today, so the overdue marker does not depend on it.
const filled = {
  actions: [
    {
      id: "0199b8e0-0000-7000-8000-0000000000d1",
      local_id: "ACT-001",
      event_id: event?.id,
      event_key: "FLY28",
      title: "Generatorenvermietung Testwil-Oberdorf anfragen und Angebote vergleichen",
      owner_user_id: ME,
      due_date: "2020-05-18",
      status: "in-progress",
      version: 2,
    },
    {
      id: "0199b8e0-0000-7000-8000-0000000000d2",
      local_id: "ACT-002",
      event_id: event?.id,
      event_key: "FLY28",
      title: "Festzelt reservieren",
      owner_user_id: ME,
      due_date: "2099-06-01",
      status: "blocked",
      version: 1,
    },
    {
      id: "0199b8e0-0000-7000-8000-0000000000d3",
      local_id: "ACT-003",
      event_id: event?.id,
      event_key: "FLY28",
      title: "Flyer drucken",
      owner_user_id: ME,
      status: "open",
      version: 1,
    },
  ],
  commitments: [
    {
      id: "0199b8e0-0000-7000-8000-0000000000e1",
      local_id: "COM-001",
      event_id: event?.id,
      event_key: "FLY28",
      text: "Lieferung des Generators am Freitag um 15:00 Uhr auf das Flugplatzgelände",
      condition: "vorbehältlich der unterzeichneten Bestellung",
      promisor: {
        kind: "institution",
        id: "0199b8e0-0000-7000-8000-0000000000f1",
        local_id: "INS-001",
        name: "Generatorenvermietung Testwil-Oberdorf Aktiengesellschaft",
      },
      owner_user_id: ME,
      due_date: "2099-05-17",
      status: "conditional",
      version: 3,
      evidence: [],
    },
  ],
  review_count: 2,
};
const empty = { actions: [], commitments: [], review_count: 0 };

async function fakeMyWork(page: Page, body: unknown): Promise<void> {
  await page.route("**/api/v1/me/work", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(body) }),
  );
}

const states = [
  { name: "filled", body: filled, ready: "Aufgaben" },
  { name: "empty", body: empty, ready: "Nichts offen" },
] as const;

for (const viewport of viewports) {
  for (const theme of themes) {
    for (const state of states) {
      test(`my work, ${state.name}, ${theme}, ${viewport.name} px: no axe violation, screenshot`, async ({
        page,
      }) => {
        await page.setViewportSize(viewport);
        await fakeSession(page);
        await fakeMyWork(page, state.body);
        await page.goto("/");
        await setTheme(page, theme);
        await expect(page.getByRole("heading", { level: 1, name: "Meine Arbeit" })).toBeVisible();
        await expect(page.getByText(state.ready, { exact: true }).first()).toBeVisible();
        if (state.name === "filled") {
          await expect(page.getByText("überfällig")).toHaveCount(1);
          expect(await textOverflows(page)).toEqual([]);
        }

        const results = await new AxeBuilder({ page })
          .withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"])
          .analyze();
        expect(results.violations).toEqual([]);

        await fontsLoaded(page);
        await expect(page).toHaveScreenshot(`my-work-${state.name}-${theme}-${viewport.name}.png`, {
          fullPage: true,
        });
      });
    }
  }
}
