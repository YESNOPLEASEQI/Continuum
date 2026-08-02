import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { AppServerApprovalDialog } from "../src/components/AppServerApprovalDialog";
import type { AppServerClientRequest } from "../src/types/models";

const { requestsMock, respondMock } = vi.hoisted(() => ({
  requestsMock: vi.fn(),
  respondMock: vi.fn(),
}));

vi.mock("../src/api/bridge", () => ({
  appApi: {
    appServerRequests: requestsMock,
    respondAppServerRequest: respondMock,
  },
}));

const baseRequest: AppServerClientRequest = {
  id: "request-1",
  continuationId: "cont-1",
  projectId: "project-1",
  threadId: "thread-1",
  turnId: "turn-1",
  itemId: "item-1",
  kind: "network",
  reason: "需要访问依赖服务",
  command: "curl https://example.com",
  cwd: "C:\\repo",
  commandActions: [],
  grantRoot: null,
  networkHost: "example.com",
  networkProtocol: "https",
  permissions: null,
  serverName: null,
  message: null,
  mode: null,
  url: null,
  requestedSchema: null,
  metadata: null,
  questions: [],
  autoResolutionMs: null,
  startedAtMs: 123,
};

describe("App Server client request relay", () => {
  beforeEach(() => {
    requestsMock.mockReset().mockResolvedValue([baseRequest]);
    respondMock.mockReset().mockResolvedValue(undefined);
  });

  it("shows approval context and returns the selected decision", async () => {
    const user = userEvent.setup();
    render(<AppServerApprovalDialog />);

    expect(
      await screen.findByRole("dialog", { name: "Codex 需要你的响应" }),
    ).toBeInTheDocument();
    expect(screen.getByText("https://example.com")).toBeInTheDocument();
    expect(screen.getByText("curl https://example.com")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "允许本次" }));

    await waitFor(() =>
      expect(respondMock).toHaveBeenCalledWith("request-1", {
        decision: "accept",
      }),
    );
  });

  it("grants only the displayed permission profile with the chosen scope", async () => {
    const user = userEvent.setup();
    const request: AppServerClientRequest = {
      ...baseRequest,
      id: "permission-1",
      kind: "permissions",
      reason: "需要读取共享工作区",
      command: null,
      networkHost: null,
      networkProtocol: null,
      permissions: { fileSystem: { read: ["C:\\shared"] } },
    };
    requestsMock.mockResolvedValue([request]);
    render(<AppServerApprovalDialog />);

    expect(await screen.findByText("权限申请")).toBeInTheDocument();
    expect(screen.getByText(/C:\\\\shared/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "本会话允许" }));

    await waitFor(() =>
      expect(respondMock).toHaveBeenCalledWith("permission-1", {
        permissions: request.permissions,
        scope: "session",
      }),
    );
  });

  it("renders a schema-backed MCP form and submits structured content", async () => {
    const user = userEvent.setup();
    requestsMock.mockResolvedValue([
      {
        ...baseRequest,
        id: "mcp-1",
        kind: "mcp_elicitation",
        reason: null,
        command: null,
        networkHost: null,
        networkProtocol: null,
        serverName: "calendar",
        message: "选择日期",
        mode: "form",
        requestedSchema: {
          type: "object",
          properties: {
            date: { type: "string", format: "date", title: "日期" },
          },
          required: ["date"],
        },
      } satisfies AppServerClientRequest,
    ]);
    render(<AppServerApprovalDialog />);

    await user.type(await screen.findByLabelText("日期 *"), "2026-08-03");
    await user.click(screen.getByRole("button", { name: "提交并继续" }));

    await waitFor(() =>
      expect(respondMock).toHaveBeenCalledWith("mcp-1", {
        action: "accept",
        content: { date: "2026-08-03" },
      }),
    );
  });

  it("collects tool user input by question id", async () => {
    const user = userEvent.setup();
    requestsMock.mockResolvedValue([
      {
        ...baseRequest,
        id: "input-1",
        kind: "tool_user_input",
        reason: null,
        command: null,
        networkHost: null,
        networkProtocol: null,
        questions: [
          {
            id: "strategy",
            header: "策略",
            question: "如何继续？",
            options: [
              { label: "安全", description: "保守执行" },
              { label: "快速", description: "优先速度" },
            ],
          },
        ],
        autoResolutionMs: 60_000,
      } satisfies AppServerClientRequest,
    ]);
    render(<AppServerApprovalDialog />);

    await user.click(await screen.findByLabelText(/安全/));
    await user.click(screen.getByRole("button", { name: "提交回答" }));

    await waitFor(() =>
      expect(respondMock).toHaveBeenCalledWith("input-1", {
        answers: { strategy: { answers: ["安全"] } },
      }),
    );
  });
});
