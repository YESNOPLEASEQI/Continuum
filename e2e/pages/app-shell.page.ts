import { type Locator, type Page } from "@playwright/test";

export class AppShellPage {
  readonly searchLink: Locator;
  readonly diagnosticsLink: Locator;
  readonly profilesLink: Locator;

  constructor(private readonly page: Page) {
    this.searchLink = page.getByRole("link", { name: "搜索与命令" });
    this.diagnosticsLink = page.getByRole("link", { name: "Diagnostics" });
    this.profilesLink = page.getByRole("link", { name: "Codex Profiles" });
  }

  async goto() {
    await this.page.goto("/projects");
    await this.page.getByRole("heading", { name: "统一项目" }).waitFor();
  }

  async openSearch() {
    await this.searchLink.click();
    await this.page.getByRole("heading", { name: "搜索与命令面板" }).waitFor();
  }
}
