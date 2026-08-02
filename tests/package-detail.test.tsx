import { render, screen } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";
import PackageDetailPage from "../src/pages/PackageDetailPage";
import { packageDetailFixture } from "./fixtures";

const { packageMock } = vi.hoisted(() => ({ packageMock: vi.fn() }));
vi.mock("../src/api/bridge", () => ({ appApi: { package: packageMock, validatePackage: vi.fn(), markResumed: vi.fn() } }));

describe("PackageDetailPage", () => {
  it("renders real package sections and recovery content", async () => {
    packageMock.mockResolvedValue(packageDetailFixture);
    render(<MemoryRouter initialEntries={["/packages/package-001"]}><Routes><Route path="/packages/:id" element={<PackageDetailPage />} /></Routes></MemoryRouter>);
    expect(await screen.findByRole("heading", { name: "完成任务包校验" })).toBeInTheDocument();
    expect(screen.getByText("任务目标")).toBeInTheDocument();
    expect(screen.getByText(/你正在接手一个由其他 AI Agent 中断的任务/)).toBeInTheDocument();
  });
});
