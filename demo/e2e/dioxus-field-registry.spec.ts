import { expect, openExample, test } from "./fixtures";

test("registry fields stay inside their pane and hide native form controls", async ({
  page,
}) => {
  const demo = await openExample(page, "/dioxus-field-registry");
  const snapshotHeading = demo.getByText("Dioform snapshot", { exact: true });
  const snapshotBox = await snapshotHeading.boundingBox();
  expect(snapshotBox).not.toBeNull();

  for (const control of [
    demo.getByPlaceholder("Ada Lovelace"),
    demo.getByPlaceholder("What are you working on?"),
  ]) {
    const box = await control.boundingBox();
    expect(box).not.toBeNull();
    expect(box!.x + box!.width).toBeLessThan(snapshotBox!.x);
  }

  const nativeSwitchInput = demo.locator('input[type="checkbox"][name="analytics"]');
  await expect(nativeSwitchInput).toHaveCount(1);
  await expect(nativeSwitchInput).toHaveCSS("position", "absolute");
  await expect(nativeSwitchInput).toHaveCSS("opacity", "0");
});

test("composition fields generate ids and show commit validation errors", async ({
  page,
}) => {
  const demo = await openExample(page, "/dioxus-field-registry");
  const input = demo.getByLabel("Display name");
  const descriptionId = await input.getAttribute("aria-describedby");

  expect(descriptionId).toBeTruthy();
  await expect(demo.locator(`#${descriptionId}`)).toHaveText(
    "The registry generates and registers the description and error ids.",
  );

  const error = input.locator("..").locator('[aria-live="polite"]');
  await expect(error).toHaveCount(1);
  await expect(error).toHaveAttribute("id", /^dxf-error-/);

  const errorId = await error.getAttribute("id");
  await input.fill("ab");
  await input.blur();

  await expect(input).toHaveAttribute("aria-invalid", "true");
  await expect(input).toHaveAttribute("aria-errormessage", errorId!);
  await expect(error).toHaveText("Use at least three characters.");
});
