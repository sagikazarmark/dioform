import { expect, test, waitForHydration } from "./fixtures";

test("application starts and hydrates", async ({ page }) => {
  const response = await page.goto("/", { waitUntil: "commit" });
  expect(response?.status()).toBe(200);
  await waitForHydration(page, 600_000);
});
