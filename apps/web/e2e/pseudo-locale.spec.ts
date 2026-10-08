import { expect, test } from "@playwright/test";
import { events, fakeEvents, fakeSession, textOverflows, unavailable, viewports } from "./fixtures";

// ADR 0024: German text can be 40 % longer than the source. No text may overflow its box or be cut
// off without a tooltip. A table never scrolls sideways (ADR 0023).
for (const viewport of viewports) {
  for (const state of [
    { name: "list", status: 200, body: { items: events } },
    { name: "empty", status: 200, body: { items: [] } },
    { name: "error", status: 503, body: unavailable },
  ]) {
    test(`pseudo-locale, ${state.name}, ${viewport.name} px: no text overflows`, async ({
      page,
    }) => {
      await page.setViewportSize(viewport);
      await fakeSession(page);
      await fakeEvents(page, state.status, state.body);
      await page.goto("/?pseudo");
      await expect(page.getByRole("heading", { name: /Áñlässé/ })).toBeVisible();
      // The skeleton is a labelled status; the live regions have no label.
      await expect(page.locator("[role=status][aria-label]")).toHaveCount(0);

      const overflows = await textOverflows(page);
      expect(overflows).toEqual([]);
    });
  }
}
