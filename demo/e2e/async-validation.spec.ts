import { expect, openExample, test } from "./fixtures";

test("debounced validation settles through the browser timer", async ({ page }) => {
  await page.clock.install();
  const demo = await openExample(page, "/validation/async");
  const username = demo.getByLabel("Username (async availability check)");
  const checking = demo.getByText("Checking availability…", { exact: true });
  const available = demo.getByText("Username is available.", { exact: true });
  const rejected = demo.getByText("That username is already taken.", { exact: true });

  const browserTime = await page.evaluate(() => Date.now());
  await page.clock.pauseAt(browserTime + 60_000);

  await username.fill("ada");
  await expect(checking).toBeVisible();
  await page.clock.runFor(499);
  await expect(checking).toBeVisible();
  await page.clock.runFor(1);
  await expect(checking).toBeHidden();
  await expect(available).toBeHidden();

  await username.blur();
  await expect(rejected).toBeVisible();

  await username.fill("marie");
  await expect(checking).toBeVisible();
  await page.clock.runFor(500);
  await expect(available).toBeVisible();
});
