import { expect, test } from "@playwright/test";
import { events, fakeEvents, unavailable, viewports } from "./fixtures";

// ADR 0024: German text can be 40 % longer than the source. No text may overflow its box or be cut
// off without a tooltip. A scroll container, for example the table at 375 px, is not an overflow.
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
      await fakeEvents(page, state.status, state.body);
      await page.goto("/?pseudo");
      await expect(page.getByRole("heading", { name: /Áñlässé/ })).toBeVisible();
      await expect(page.getByRole("status")).toHaveCount(0);

      const overflows = await page.evaluate(() => {
        const found: string[] = [];
        for (const element of document.querySelectorAll<HTMLElement>("body *")) {
          const style = getComputedStyle(element);
          const scrolls = ["auto", "scroll"].includes(style.overflowX);
          if (
            scrolls ||
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
      expect(overflows).toEqual([]);
    });
  }
}
