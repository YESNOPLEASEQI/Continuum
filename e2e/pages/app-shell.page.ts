import { type Locator, type Page } from "@playwright/test";

export class AppShellPage {
  readonly menuButton: Locator;
  readonly searchButton: Locator;
  readonly diagnosticsButton: Locator;
  readonly profilesButton: Locator;

  constructor(private readonly page: Page) {
    this.menuButton = page.getByRole("button", { name: "打开全局菜单" });
    this.searchButton = page.getByRole("button", { name: /搜索与命令.*Search/i });
    this.diagnosticsButton = page.getByRole("button", { name: /Diagnostics/i });
    this.profilesButton = page.getByRole("button", { name: /Profiles/i });
  }

  async goto() {
    await this.page.goto("/projects");
    await this.page.getByRole("heading", { name: "Continuum", level: 1 }).waitFor();
  }

  async openMenu() {
    await this.menuButton.click();
    await this.page.getByRole("dialog", { name: "Continuum 全局菜单" }).waitFor();
  }

  async openSearch() {
    await this.openMenu();
    await this.searchButton.click();
    await this.page.getByRole("heading", { name: "搜索与命令面板" }).waitFor();
  }
}
