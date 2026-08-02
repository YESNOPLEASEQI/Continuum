import { expect, test } from "@playwright/test";

test("browser shell exposes Continuum primary navigation without legacy package routes", async ({
  page,
}) => {
  await page.goto("/");
  await expect(page).toHaveTitle("Continuum");
  await expect(page).toHaveURL(/\/projects$/);
  await expect(
    page.getByRole("heading", { name: "Continuum", level: 1 }),
  ).toBeVisible();
  await page.getByRole("button", { name: "打开全局菜单" }).click();
  await expect(page.getByRole("button", { name: /项目档案.*Projects/i })).toBeVisible();
  await expect(page.getByRole("button", { name: /来源会话.*Sessions/i })).toBeVisible();
  await expect(page.getByRole("button", { name: /Skills \/ MCP/i })).toBeVisible();
  await expect(page.getByRole("link", { name: "任务包" })).toHaveCount(0);
  await expect(
    page.getByText(/App Server \/ CLI fallback \/ SQLite v4/),
  ).toBeVisible();
});
