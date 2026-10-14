import { expect, test } from "@playwright/test";

test("a stored theme applies before the app starts, and the toggle changes it", async ({
  page,
}) => {
  // The init script runs before the first paint, so the attribute is there when the app mounts.
  await page.addInitScript(() => window.localStorage.setItem("tada-theme", "dark"));
  await page.goto("/_gallery");
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");

  await page.getByRole("button", { name: "Farbschema: Dunkel" }).click();
  await expect(page.locator("html")).not.toHaveAttribute("data-theme");
  expect(await page.evaluate(() => window.localStorage.getItem("tada-theme"))).toBeNull();
});
