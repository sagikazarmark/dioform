import { expect, test } from "./fixtures";

test("application starts and hydrates", async ({ page }) => {
  const response = await page.goto("/", { waitUntil: "commit" });
  expect(response?.status()).toBe(200);
  await expect(page.locator('[data-demo-hydrated="true"]')).toBeVisible({
    timeout: 600_000,
  });
});
