import { type Locator, type Page } from "@playwright/test";

export class DashboardPage {
  readonly heading: Locator;
  readonly scanButton: Locator;
  constructor(private readonly page: Page) {
    this.heading = page.getByRole("heading", { name: "统一项目" });
    this.scanButton = page.getByTestId("scan-sessions-btn");
  }
  async goto() {
    await this.page.goto("/projects");
    await this.heading.waitFor();
  }
}
