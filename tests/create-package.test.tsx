import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";
import CreatePackagePage from "../src/pages/CreatePackagePage";
import { useAppStore } from "../src/store/appStore";

vi.mock("../src/api/bridge", () => ({ appApi: { packageDraft: vi.fn(), createPackage: vi.fn(), sessions: vi.fn().mockResolvedValue([]) } }));

describe("CreatePackagePage", () => {
  it("validates required handoff fields before writing", async () => {
    useAppStore.setState({ sessions: [], loadSessions: vi.fn().mockResolvedValue(undefined) });
    render(<MemoryRouter initialEntries={["/packages/new"]}><CreatePackagePage /></MemoryRouter>);
    await userEvent.click(screen.getByTestId("create-package-submit"));
    expect(await screen.findByText("请选择来源会话")).toBeInTheDocument();
    expect(screen.getByText("标题至少需要 2 个字符")).toBeInTheDocument();
    expect(screen.getByText("请填写原始目标")).toBeInTheDocument();
    expect(screen.getByText("至少需要一项下一步操作")).toBeInTheDocument();
  });
});
