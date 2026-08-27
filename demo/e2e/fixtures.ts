import { expect, test as base, type Locator, type Page } from "@playwright/test";

export const test = base.extend<{ browserHealth: void }>({
  browserHealth: [
    async ({ page }, use, testInfo) => {
      const errors: string[] = [];

      page.on("pageerror", (error) => {
        errors.push(`Uncaught page error: ${error.message}`);
      });
      page.on("console", (message) => {
        const isMissingFavicon =
          message.location().url.endsWith("/favicon.ico") &&
          message.text().includes("404");
        if (message.type() === "error" && !isMissingFavicon) {
          errors.push(`Console error: ${message.text()}`);
        }
      });
      page.on("requestfailed", (request) => {
        errors.push(
          `Request failed: ${request.method()} ${request.url()} ${request.failure()?.errorText ?? ""}`,
        );
      });

      await use();

      if (errors.length > 0) {
        await testInfo.attach("browser-errors", {
          body: errors.join("\n"),
          contentType: "text/plain",
        });
      }
      expect(errors, errors.join("\n")).toEqual([]);
    },
    { auto: true },
  ],
});

export async function openExample(page: Page, path: string) {
  const response = await page.goto(path);
  expect(response?.status()).toBe(200);
  await waitForHydration(page);

  const demo = page.getByRole("region", { name: "Example demo" });
  await expect(demo).toHaveCount(1);

  return demo;
}

export function stateValue(demo: Locator, label: string) {
  return demo
    .locator("dt")
    .filter({ hasText: label })
    .locator("xpath=following-sibling::dd[1]");
}

export async function waitForHydration(page: Page, timeout = 10_000) {
  await expect(page.locator('[data-demo-hydrated="true"]')).toBeVisible({
    timeout,
  });

  // Dioxus can briefly retain the SSR tree beside the hydrated client tree.
  await expect(page.locator("[data-demo-hydrated]")).toHaveCount(1, {
    timeout,
  });
}

export { expect };
