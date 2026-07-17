import { expect, openExample, test } from "./fixtures";

test("server rejection becomes a field submit error", async ({ page }) => {
  const demo = await openExample(page, "/server");

  await demo
    .getByLabel("Email (try taken@example.com)")
    .fill("taken@example.com");
  await demo.getByRole("button", { name: "Check on the server" }).click();

  await expect(
    demo.getByText("That email is already registered.", { exact: true }),
  ).toBeVisible();
  await expect(page).toHaveURL(/\/server$/);
});
