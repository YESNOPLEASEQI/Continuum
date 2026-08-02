import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";
import UnifiedChatPage from "../src/pages/UnifiedChatPage";
import type {
  ConversationNode,
  UnifiedProjectDetail,
} from "../src/types/models";

const { projectMock, timelineMock } = vi.hoisted(() => ({
  projectMock: vi.fn(),
  timelineMock: vi.fn(),
}));
vi.mock("../src/api/bridge", () => ({
  appApi: {
    project: projectMock,
    timeline: timelineMock,
    pollSessionChanges: vi.fn(),
    addNote: vi.fn(),
    createBranch: vi.fn(),
    compileContext: vi.fn(),
    updateNode: vi.fn(),
  },
}));

const health = {
  level: "healthy" as const,
  messageCount: 2,
  estimatedTokens: 120,
  duplicateRatio: 0,
  toolLogRatio: 0.2,
  staleRatio: 0,
  incorrectRatio: 0,
  conflictCount: 0,
  uncompressedLogCount: 0,
  contextBudget: 32000,
  thresholdRatio: 0.01,
  lastSnapshotAt: null,
  lastFreshContinuationAt: null,
  currentSessionDurationSeconds: 60,
  reasons: ["健康"],
};
const project: UnifiedProjectDetail = {
  id: "project-1",
  name: "Continuum",
  projectPath: "C:\\work\\continuum",
  gitRepository: null,
  goal: "Unify sessions",
  currentTask: "Filter the timeline",
  currentBranchId: "branch-1",
  currentBranchName: "main",
  defaultAgent: "codex",
  defaultModel: "default",
  sessionCount: 1,
  updatedAt: "2026-08-01T00:00:00Z",
  archived: false,
  pathExists: true,
  health,
  constraints: [],
  branches: [
    {
      id: "branch-1",
      projectId: "project-1",
      name: "main",
      parentBranchId: null,
      forkNodeId: null,
      status: "active",
      createdAt: "2026-08-01T00:00:00Z",
      updatedAt: "2026-08-01T00:00:00Z",
      nodeCount: 2,
    },
  ],
  sessions: [
    {
      id: "session-1",
      agent: "codex",
      title: "Source",
      sourcePath: "source.jsonl",
      branchId: "branch-1",
      messageCount: 2,
      lastSyncedAt: "2026-08-01T00:00:00Z",
      continuationId: null,
    },
  ],
  activeFiles: [],
  decisions: [],
  todos: [],
  gitState: null,
};
const baseNode = {
  projectId: "project-1",
  parentNodeId: null,
  branchId: "branch-1",
  sourceAgent: "codex" as const,
  sourceSessionId: "session-1",
  createdAt: "2026-08-01T00:00:00Z",
  importance: 50,
  status: "active" as const,
  metadata: { role: "assistant" },
};
const nodes: ConversationNode[] = [
  {
    ...baseNode,
    id: "node-1",
    nodeType: "message",
    content: "Timeline is ready",
  },
  {
    ...baseNode,
    id: "node-2",
    nodeType: "tool_call",
    content: "git status --short\n M src/App.tsx",
  },
];

describe("Unified timeline", () => {
  it("filters indexed nodes and exposes raw records", async () => {
    projectMock.mockResolvedValue(project);
    timelineMock.mockResolvedValue(nodes);
    const user = userEvent.setup();
    render(
      <MemoryRouter
        initialEntries={[
          "/projects/project-1/chat?branch=branch-1&node=node-1",
        ]}
      >
        <Routes>
          <Route path="/projects/:id/chat" element={<UnifiedChatPage />} />
        </Routes>
      </MemoryRouter>,
    );
    expect(
      await screen.findByRole("heading", { name: "Continuum" }),
    ).toBeInTheDocument();
    expect(screen.getAllByText("原始记录")).toHaveLength(2);
    await user.type(
      screen.getByPlaceholderText("在当前分支中搜索"),
      "git status",
    );
    expect(screen.getAllByText(/git status --short/).length).toBeGreaterThan(0);
    expect(screen.queryByText("Timeline is ready")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /Graph/ }));
    expect(
      screen.getByRole("heading", { name: "分支与会话链" }),
    ).toBeInTheDocument();
    expect(screen.getByText(/SQLite 中的真实 branch/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /Files/ }));
    expect(screen.getByRole("heading", { name: "Files" })).toBeInTheDocument();
  });
});
