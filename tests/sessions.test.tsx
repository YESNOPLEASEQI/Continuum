import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import SessionsPage from "../src/pages/SessionsPage";
import { useAppStore } from "../src/store/appStore";
import { sessionFixture } from "./fixtures";

vi.mock("../src/api/bridge", () => ({ appApi: { sessions: vi.fn().mockResolvedValue([]), projects: vi.fn().mockResolvedValue([]), scan: vi.fn().mockResolvedValue([]), dashboard: vi.fn() } }));

describe("SessionsPage", () => {
  beforeEach(() => useAppStore.setState({ sessions: [], projects: [], loading: false, scanning: false, error: null, loadProjects: vi.fn().mockResolvedValue(undefined) }));
  it("renders indexed session behavior and metadata", async () => {
    useAppStore.setState({ sessions: [sessionFixture], loadSessions: vi.fn().mockResolvedValue(undefined) });
    render(<MemoryRouter><SessionsPage /></MemoryRouter>);
    expect(screen.getByText("修复会话解析器")).toBeInTheDocument();
    expect(screen.getByText(/12 消息/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /压缩后新建会话/ })).toBeDisabled();
  });
  it("shows an actionable empty state", () => {
    useAppStore.setState({ sessions: [], loadSessions: vi.fn().mockResolvedValue(undefined) });
    render(<MemoryRouter><SessionsPage /></MemoryRouter>);
    expect(screen.getByText("未发现 Codex 会话")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "扫描默认目录" })).toBeInTheDocument();
  });
  it("surfaces backend errors without a blank screen", () => {
    useAppStore.setState({ error: "SQLite 文件被占用", loadSessions: vi.fn().mockResolvedValue(undefined) });
    render(<MemoryRouter><SessionsPage /></MemoryRouter>);
    expect(screen.getByRole("alert")).toHaveTextContent("SQLite 文件被占用");
  });
});
