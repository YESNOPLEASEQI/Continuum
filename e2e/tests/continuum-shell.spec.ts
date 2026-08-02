import { expect, test } from "@playwright/test";

test("browser shell exposes Continuum primary navigation without legacy package routes", async ({
  page,
}) => {
  await page.goto("/");
  await expect(page).toHaveTitle("Continuum");
  await expect(page).toHaveURL(/\/projects$/);
  await expect(
    page.getByText("Continuum", { exact: true }).first(),
  ).toBeVisible();
  await expect(page.getByRole("link", { name: "统一项目" })).toBeVisible();
  await expect(
    page.getByRole("link", { name: "会话", exact: true }),
  ).toBeVisible();
  await expect(page.getByRole("link", { name: "Skills 与配置" })).toBeVisible();
  await expect(page.getByRole("link", { name: "任务包" })).toHaveCount(0);
  await expect(
    page.getByText("此操作需要在 Continuum 桌面客户端中运行").first(),
  ).toBeVisible();
});
