import { expect, test } from "@playwright/test";
import { events, fakeEvent, fakeSession } from "./fixtures";

const event = events[0];

test("the client sends a content security policy that breaks no screen and blocks a remote image", async ({
  page,
}) => {
  const violations: string[] = [];
  page.on("console", (message) => {
    if (message.text().includes("Content Security Policy")) {
      violations.push(message.text());
    }
  });
  await fakeSession(page);
  await fakeEvent(page, event);

  const response = await page.goto(`/events/${event?.id}`);
  expect(response?.headers()["content-security-policy"]).toContain("img-src 'self'");
  await expect(page.getByRole("heading", { level: 1 })).toBeVisible();
  // The fonts, the styles and the scripts of the client all load under the policy.
  expect(violations).toEqual([]);

  // A remote image, for example from a draft that an agent wrote, never loads (ADR 0058).
  const blocked = await page.evaluate(
    () =>
      new Promise<string>((resolve) => {
        document.addEventListener("securitypolicyviolation", (e) => resolve(e.violatedDirective), {
          once: true,
        });
        const image = new Image();
        image.src = "https://example.org/pixel.png";
        document.body.append(image);
      }),
  );
  expect(blocked).toMatch(/^img-src/);
});
