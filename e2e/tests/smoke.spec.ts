import { expect, test } from "@playwright/test";
import { DashboardPage } from "../pages/dashboard.page";

test("Continuum exposes local-only project shell and scan control", async ({
  page,
}) => {
  const dashboard = new DashboardPage(page);
  await dashboard.goto();
  await expect(dashboard.wordmark).toBeVisible();
  await expect(dashboard.heading).toBeVisible();
  await expect(page.getByText(/SQLite error/i)).toBeVisible();
  await expect(dashboard.scanButton).toBeEnabled();
  await expect(dashboard.sessionsButton).toBeEnabled();
  expect(await dashboard.hasHorizontalOverflow()).toBe(false);
});

test("Continuum entrance remains usable with reduced motion", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  const dashboard = new DashboardPage(page);
  await dashboard.goto();
  await expect(dashboard.wordmark).toBeVisible();
  await expect(dashboard.sessionsButton).toBeEnabled();
  expect(await dashboard.hasHorizontalOverflow()).toBe(false);
});
