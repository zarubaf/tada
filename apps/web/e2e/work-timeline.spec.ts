import { expect, type Page, test } from "@playwright/test";
import { fakeProfile, textOverflows } from "./fixtures";
import { actions, event, fakeWork, milestones } from "./workFixtures";

// The plan of the timeline view of „Arbeit“: how it scrolls, what the keyboard reaches and how a
// diamond reacts. The screenshots of the view are in `work-views.spec.ts`.

const timeline = `/events/${event?.id}/work?view=timeline`;

// The sample has two actions in 2020, which would stretch the axis over decades: the plan shows the
// same actions near the others.
const near = actions.map((action: Record<string, unknown>) =>
  String(action.due_date).startsWith("2020") ? { ...action, due_date: "2099-05-10" } : action,
);

const eventWindow = {
  id: "f1",
  field_id: "fd1",
  field_key: "date_window",
  state: "assumption",
  value: { type: "date-window", start: "2099-05-14", end: "2099-06-02", granularity: "day" },
  redacted: false,
  version: 1,
  evidence: [],
  accepted_by: { kind: "member", id: "x", channel: "web" },
  accepted_at: "2030-03-01T13:12:00Z",
};

async function fakeTimeline(page: Page, lists: Parameters<typeof fakeWork>[1] = {}) {
  await fakeWork(page, { actions: near, ...lists });
  await fakeProfile(page, { facts: [eventWindow], proposals: [], open_questions: [] }, []);
  // Today is a day on the axis, so the plan does not depend on the real date.
  await page.clock.setFixedTime(new Date("2099-05-08T10:00:00Z"));
}

const pageScrollsSideways = (page: Page) =>
  page.evaluate(() => document.documentElement.scrollWidth > window.innerWidth);

test("a span of years scrolls inside the timeline and never the page", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await fakeTimeline(page, {
    actions: [{ ...actions[0], start_date: "2099-05-06", due_date: "2102-05-01" }],
  });
  await page.goto(timeline);
  const axis = page.getByRole("region", { name: "Zeitachse" });
  await expect(axis).toBeVisible();

  const sizes = await axis.evaluate((node) => ({
    inner: node.scrollWidth,
    outer: node.clientWidth,
  }));
  expect(sizes.inner).toBeGreaterThan(sizes.outer);
  expect(await pageScrollsSideways(page)).toBe(false);
});

test("at 320 px the timeline shows the list by week without a sideways scroll", async ({
  page,
}) => {
  await page.setViewportSize({ width: 320, height: 800 });
  await fakeTimeline(page);
  await page.goto(timeline);
  await expect(page.getByRole("region", { name: "Nach Wochen" })).toBeVisible();
  await expect(page.getByRole("region", { name: "Zeitachse" })).toBeHidden();
  expect(await pageScrollsSideways(page)).toBe(false);
  expect(await textOverflows(page)).toEqual([]);
});

test("wide: the axis takes the keyboard and the list stays in the accessibility tree", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await fakeTimeline(page);
  await page.goto(timeline);
  const axis = page.getByRole("region", { name: "Zeitachse" });
  await expect(axis).toBeVisible();

  // The list is out of sight, but a screen reader finds it by its role and name.
  const list = page.getByRole("region", { name: "Nach Wochen" });
  await expect(list).toHaveCount(1);
  await expect(list.getByRole("link", { name: /ACT-001/ })).toHaveAttribute("tabindex", "-1");
  await expect(axis.getByRole("link", { name: /ACT-001/ })).not.toHaveAttribute("tabindex", "-1");

  // Tab from the axis goes through its links only: no focused link lies in the hidden list.
  await axis.focus();
  for (let step = 0; step < 4; step += 1) {
    await page.keyboard.press("Tab");
    expect(
      await page.evaluate(() => document.activeElement?.closest("section")?.ariaLabel),
    ).not.toBe("Nach Wochen");
  }
});

test("narrow: the axis leaves the keyboard and the accessibility tree", async ({ page }) => {
  await page.setViewportSize({ width: 375, height: 800 });
  await fakeTimeline(page);
  await page.goto(timeline);
  const list = page.getByRole("region", { name: "Nach Wochen" });
  await expect(list).toBeVisible();
  await expect(page.getByRole("region", { name: "Zeitachse" })).toHaveCount(0);
  await expect(list.getByRole("link", { name: /ACT-001/ })).not.toHaveAttribute("tabindex", "-1");
});

for (const contrast of ["no-preference", "more"] as const) {
  test(`hovering a milestone diamond changes its border, whatever its status (contrast ${contrast})`, async ({
    page,
  }) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.emulateMedia({ contrast });
    const [open] = milestones;
    const reached = { ...open, id: "m-reached", local_id: "MIL-002", status: "reached" };
    const canceled = { ...open, id: "m-canceled", local_id: "MIL-003", status: "canceled" };
    await fakeTimeline(page, { milestones: [open, reached, canceled] });
    await page.goto(timeline);
    const axis = page.getByRole("region", { name: "Zeitachse" });
    // Open, reached and canceled. A screenshot cannot show a hover, so read the styles.
    for (const id of ["MIL-001", "MIL-002", "MIL-003"]) {
      const link = axis.getByRole("link", { name: new RegExp(id) });
      const diamond = link.locator("[data-status]");
      const border = () =>
        diamond.evaluate((el) => {
          const style = getComputedStyle(el);
          return `${style.borderTopColor} ${style.borderTopWidth}`;
        });
      await page.mouse.move(0, 0);
      const rest = await border();
      await link.hover();
      await expect.poll(border).not.toBe(rest);
    }
  });
}
