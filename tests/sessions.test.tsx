import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import SessionsPage from "../src/pages/SessionsPage";
import { ContinuumMotionProvider } from "../src/motion/ContinuumMotion";
import { useAppStore } from "../src/store/appStore";
import { sessionFixture } from "./fixtures";

vi.mock("../src/api/bridge", () => ({ appApi: { sessions: vi.fn().mockResolvedValue([]), projects: vi.fn().mockResolvedValue([]), scan: vi.fn().mockResolvedValue([]), dashboard: vi.fn() } }));

function renderSessions() {
  return render(
    <MemoryRouter>
      <ContinuumMotionProvider>
        <SessionsPage />
      </ContinuumMotionProvider>
    </MemoryRouter>,
  );
}

describe("SessionsPage", () => {
  beforeEach(() => useAppStore.setState({ sessions: [], projects: [], loading: false, scanning: false, error: null, loadProjects: vi.fn().mockResolvedValue(undefined) }));
  it("renders indexed session behavior and metadata", async () => {
    useAppStore.setState({ sessions: [sessionFixture], loadSessions: vi.fn().mockResolvedValue(undefined) });
    renderSessions();
    expect(screen.getByText("修复会话解析器")).toBeInTheDocument();
    expect(screen.getByText("Codex Desktop")).toBeInTheDocument();
    expect(screen.getByText("agentpack")).toBeInTheDocument();
    expect(screen.getByText("未绑定")).toBeInTheDocument();
    expect(screen.queryByText(/12 消息/)).not.toBeInTheDocument();
    expect(screen.queryByText("其他 Agent：未来扩展")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "新建续接" })).toBeDisabled();
    expect(screen.getByText("继续原会话")).toBeInTheDocument();
  });
  it("shows an actionable empty state", () => {
    useAppStore.setState({ sessions: [], loadSessions: vi.fn().mockResolvedValue(undefined) });
    renderSessions();
    expect(screen.getByText("未发现 Codex 会话")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "扫描默认目录" })).toBeInTheDocument();
  });
  it("surfaces backend errors without a blank screen", () => {
    useAppStore.setState({ error: "SQLite 文件被占用", loadSessions: vi.fn().mockResolvedValue(undefined) });
    renderSessions();
    expect(screen.getByRole("alert")).toHaveTextContent("SQLite 文件被占用");
  });
});
