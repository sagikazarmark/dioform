import { expect, openExample, stateValue, test } from "./fixtures";

test("happy path validates, submits, and exposes focused state", async ({ page }) => {
  const demo = await openExample(page, "/happy-path");
  const name = demo.getByLabel("Name");
  const email = demo.getByLabel("Email");
  const terms = demo.getByRole("switch", {
    name: "I agree to the code of conduct",
  });

  await expect(stateValue(demo, "name.blurred")).toHaveText("false");

  await demo.getByRole("button", { name: "Register" }).click();

  await expect(name).toHaveAttribute("aria-invalid", "true");
  await expect(email).toHaveAttribute("aria-invalid", "true");
  await expect(terms).toHaveAttribute("aria-invalid", "true");
  await expect(
    demo.getByText("Accept the code of conduct to continue."),
  ).toBeVisible();
  await expect(stateValue(demo, "submit.status")).toHaveText("blocked");
  await expect(stateValue(demo, "submit.attempts")).toHaveText("1");
  await expect(stateValue(demo, "name.visible_errors")).toHaveText("1");

  await name.fill("Ada Lovelace");
  await email.fill("ada@example.com");
  await expect(stateValue(demo, "name.blurred")).toHaveText("true");
  await expect(stateValue(demo, "name.visible_errors")).toHaveText("0");
  await demo.getByRole("radio", { name: "Remote" }).click();
  await demo.getByRole("switch", { name: "Product updates" }).click();
  await terms.click();
  await demo.getByRole("button", { name: "Register" }).click();

  await expect(stateValue(demo, "submit.status")).toHaveText("succeeded");
  await expect(stateValue(demo, "submit.attempts")).toHaveText("2");
  await expect(stateValue(demo, "form.dirty")).toHaveText("true");
  await expect(stateValue(demo, "name.touched")).toHaveText("true");
  await expect(stateValue(demo, "track")).toHaveText("-");
  await expect(stateValue(demo, "product_updates")).toHaveText("true");
  await expect(
    demo.getByText("Ada Lovelace registered for remote attendance."),
  ).toBeVisible();

  await demo.getByLabel("Track (optional)").click();
  await demo.getByRole("option", { name: "Backend systems" }).click();
  await expect(stateValue(demo, "track")).toHaveText("Backend systems");
});

test("happy path stacks cleanly on a mobile viewport", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  const demo = await openExample(page, "/happy-path");
  const nameBox = await demo.getByLabel("Name").boundingBox();
  const emailBox = await demo.getByLabel("Email").boundingBox();

  expect(nameBox).not.toBeNull();
  expect(emailBox).not.toBeNull();
  expect(Math.abs(nameBox!.x - emailBox!.x)).toBeLessThan(2);
  expect(emailBox!.y).toBeGreaterThan(nameBox!.y + nameBox!.height);
  expect(
    await demo.evaluate((root) => {
      const rootRight = root.getBoundingClientRect().right;
      return Array.from(root.querySelectorAll<HTMLElement>("*"))
        .filter((element) => element.getBoundingClientRect().right > rootRight + 1)
        .map((element) => `${element.tagName.toLowerCase()}.${element.className}`);
    }),
  ).toEqual([]);
});
