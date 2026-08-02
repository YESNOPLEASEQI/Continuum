import { expect, test } from "@playwright/test";

test("Continuum exposes local-only project shell and scan control", async ({
  page,
}) => {
  await page.goto("/projects");
  await expect(page.getByRole("heading", { name: "统一项目" })).toBeVisible();
  await expect(page.getByText("无网络传输")).toBeVisible();
  await expect(page.getByTestId("scan-sessions-btn")).toBeEnabled();
});
