import { expect, test } from "@playwright/test";
import { AppShellPage } from "../pages/app-shell.page";

test("primary product routes and keyboard search remain reachable", async ({
  page,
}) => {
  const shell = new AppShellPage(page);
  await shell.goto();
  await shell.openMenu();
  await expect(shell.searchButton).toBeVisible();
  await expect(shell.diagnosticsButton).toBeVisible();
  await expect(shell.profilesButton).toBeVisible();
  await page.getByRole("button", { name: "关闭全局菜单" }).click();
  await shell.openSearch();
  await expect(page.getByPlaceholder("输入搜索内容或命令…")).toBeFocused();
  await page.keyboard.press("Control+K");
  await expect(page).toHaveURL(/\/search$/);
});
