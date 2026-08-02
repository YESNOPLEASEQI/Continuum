import { expect, test } from "@playwright/test";
import { AppShellPage } from "../pages/app-shell.page";

test("primary product routes and keyboard search remain reachable", async ({
  page,
}) => {
  const shell = new AppShellPage(page);
  await shell.goto();
  await expect(shell.searchLink).toBeVisible();
  await expect(shell.diagnosticsLink).toBeVisible();
  await expect(shell.profilesLink).toBeVisible();
  await shell.openSearch();
  await expect(page.getByPlaceholder("输入搜索内容或命令…")).toBeFocused();
  await page.keyboard.press("Control+K");
  await expect(page).toHaveURL(/\/search$/);
});
