import { expect, openExample, test } from "./fixtures";

test("browser file selection participates in managed submission", async ({ page }) => {
  const demo = await openExample(page, "/files");
  const submit = demo.getByRole("button", { name: "Submit" });

  await expect(
    demo.getByText("No file selected.", { exact: true }),
  ).toBeVisible();

  await submit.click();
  await expect(
    demo.getByText("Blocked: the file field is required.", { exact: true }),
  ).toBeVisible();

  await demo.getByLabel("Avatar (file field)").setInputFiles({
    name: "avatar.png",
    mimeType: "image/png",
    buffer: Buffer.from(
      "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=",
      "base64",
    ),
  });

  await expect(demo.getByText("avatar.png")).toBeVisible();
  await expect(demo.getByText("image/png")).toBeVisible();

  await submit.click();
  await expect(
    demo.getByText("Submitted with the selected file.", { exact: true }),
  ).toBeVisible();
});
