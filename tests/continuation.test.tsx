import { render, screen } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";
import NewContinuationPage from "../src/pages/NewContinuationPage";
import type {
  CompiledContext,
  UnifiedProjectDetail,
} from "../src/types/models";

const { projectMock, compileMock } = vi.hoisted(() => ({
  projectMock: vi.fn(),
  compileMock: vi.fn(),
}));
vi.mock("../src/api/bridge", () => ({
  appApi: {
    project: projectMock,
    compileContext: compileMock,
    createContinuation: vi.fn(),
    launchContinuation: vi.fn(),
    pollContinuation: vi.fn(),
    bindContinuation: vi.fn(),
  },
}));

const health = {
  level: "growing" as const,
  messageCount: 84,
  estimatedTokens: 84200,
  duplicateRatio: 0.12,
  toolLogRatio: 0.2,
  staleRatio: 0.04,
  incorrectRatio: 0,
  conflictCount: 0,
  uncompressedLogCount: 0,
  contextBudget: 32000,
  thresholdRatio: 2.63,
  lastSnapshotAt: null,
  lastFreshContinuationAt: null,
  currentSessionDurationSeconds: null,
  reasons: ["上下文开始增长"],
};
const project: UnifiedProjectDetail = {
  id: "project-1",
  name: "Continuum",
  projectPath: "C:\\work\\continuum",
  gitRepository: null,
  goal: "Ship Fresh Continuation",
  currentTask: "Detect the new session",
  currentBranchId: "branch-1",
  currentBranchName: "main",
  defaultAgent: "codex",
  defaultModel: "default",
  sessionCount: 2,
  updatedAt: "2026-07-31T00:00:00Z",
  archived: false,
  pathExists: true,
  health,
  constraints: ["Do not use resume for Fresh"],
  branches: [],
  sessions: [],
  activeFiles: [],
  decisions: [],
  todos: [],
  gitState: null,
};
const compiled: CompiledContext = {
  projectId: "project-1",
  branchId: "branch-1",
  targetAgent: "codex",
  targetModel: "default",
  tokenBudget: 32000,
  estimatedTokens: 15600,
  originalEstimatedTokens: 84200,
  contentHash: "abc123",
  generatedAt: "2026-07-31T00:01:00Z",
  systemContext: "Fresh continuation",
  compiledText: "# Current Phase\nDetect the new session",
  items: [],
  conflicts: [],
  health,
};

describe("Fresh Continuation", () => {
  it("makes Fresh primary while keeping Resume and Fork distinct", async () => {
    projectMock.mockResolvedValue(project);
    compileMock.mockResolvedValue(compiled);
    render(
      <MemoryRouter initialEntries={["/projects/project-1/continuation"]}>
        <Routes>
          <Route
            path="/projects/:id/continuation"
            element={<NewContinuationPage />}
          />
        </Routes>
      </MemoryRouter>,
    );
    expect(
      await screen.findByRole("heading", { name: "压缩后开启干净会话" }),
    ).toBeInTheDocument();
    expect(screen.getByText("恢复原会话")).toBeInTheDocument();
    expect(screen.getByText("从原历史分叉")).toBeInTheDocument();
    expect(
      screen.getByText("新建干净会话，仅注入编译后的上下文。"),
    ).toBeInTheDocument();
    expect(
      await screen.findByRole("button", { name: /启动 Fresh Continuation/ }),
    ).toBeEnabled();
  });
});
