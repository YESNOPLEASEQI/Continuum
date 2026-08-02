import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { ProjectArchiveDeck } from "../src/components/ProjectArchiveDeck";
import type { UnifiedProjectSummary } from "../src/types/models";
import { sessionFixture } from "./fixtures";

const project: UnifiedProjectSummary = {
  id: "project-continuum",
  name: "Continuum Desktop",
  projectPath: "C:\\work\\continuum",
  gitRepository: "C:\\work\\continuum",
  goal: "Keep local Codex work coherent",
  currentTask: "Rebuild the desktop interaction model",
  currentBranchId: "branch-main",
  currentBranchName: "main",
  defaultAgent: "codex",
  defaultModel: "default",
  sessionCount: 12,
  updatedAt: "2026-08-02T12:00:00Z",
  archived: false,
  pathExists: true,
  health: {
    level: "growing",
    messageCount: 86,
    estimatedTokens: 18400,
    duplicateRatio: 0.04,
    toolLogRatio: 0.21,
    staleRatio: 0.08,
    incorrectRatio: 0,
    conflictCount: 0,
    uncompressedLogCount: 2,
    contextBudget: 32000,
    thresholdRatio: 0.58,
    lastSnapshotAt: null,
    lastFreshContinuationAt: null,
    currentSessionDurationSeconds: 3600,
    reasons: ["The active context is growing"],
  },
};

describe("ProjectArchiveDeck", () => {
  it("renders real project and session readouts and opens the selected archive", async () => {
    const user = userEvent.setup();
    const onOpenProject = vi.fn();
    const boundSession = {
      ...sessionFixture,
      title: "Refine the Continuum workspace",
      boundProjectId: project.id,
      boundProjectName: project.name,
    };

    render(
      <ProjectArchiveDeck
        projects={[project]}
        sessions={[boundSession]}
        loading={false}
        error={null}
        onRetry={vi.fn()}
        onOpenProject={onOpenProject}
        onOpenSession={vi.fn()}
        onBrowseSessions={vi.fn()}
        onCreateProject={vi.fn()}
        onImportProject={vi.fn()}
      />,
    );

    expect(screen.getByRole("heading", { name: "Continuum" })).toBeInTheDocument();
    expect(screen.getAllByText("Continuum Desktop")).toHaveLength(2);
    expect(screen.getAllByText("Refine the Continuum workspace")).toHaveLength(2);
    expect(screen.getByText("58% context")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /Open archive/ }));
    expect(onOpenProject).toHaveBeenCalledWith(project);
  });
});
