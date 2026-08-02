import { type Locator, type Page } from "@playwright/test";

export class DashboardPage {
  readonly heading: Locator;
  readonly wordmark: Locator;
  readonly scanButton: Locator;
  readonly sessionsButton: Locator;
  constructor(private readonly page: Page) {
    this.heading = page.getByRole("heading", { name: "Continuum", level: 1 });
    this.wordmark = page.getByRole("heading", { name: "Continuum", level: 1 });
    this.scanButton = page.getByTestId("scan-sessions-btn");
    this.sessionsButton = page.getByRole("button", { name: /浏览来源会话/ });
  }
  async goto() {
    await this.page.goto("/projects");
    await this.wordmark.waitFor();
  }

  async hasHorizontalOverflow() {
    return this.page.evaluate(
      () =>
        document.documentElement.scrollWidth >
        document.documentElement.clientWidth,
    );
  }
}
