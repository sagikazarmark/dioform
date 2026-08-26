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
