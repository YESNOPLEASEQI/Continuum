import type { AppSettings, PackageDetail, SessionSummary } from "../src/types/models";

export const sessionFixture: SessionSummary = {
  id: "session-001", title: "修复会话解析器", agent: "codex", createdAt: "2026-07-29T10:00:00Z", updatedAt: "2026-07-30T10:00:00Z",
  workingDirectory: "C:\\work\\agentpack", gitRepository: "C:\\work\\agentpack", messageCount: 12, toolCallCount: 4,
  hasFileChanges: true, canPackage: true, sourcePath: "C:\\Users\\test\\.codex\\sessions\\session-001.jsonl", parseWarning: null,
};

export const packageDetailFixture: PackageDetail = {
  id: "package-001", title: "完成任务包校验", sourceAgent: "codex", targetAgent: "claude", createdAt: "2026-07-30T12:00:00Z",
  projectPath: "C:\\work\\agentpack", packagePath: "C:\\packs\\package-001", schemaVersion: "1.0.0-alpha.1", integrity: "valid", hasPatch: true,
  securityWarningCount: 0, imported: false, resumed: false,
  manifest: { schemaVersion: "1.0.0-alpha.1", packageId: "package-001", title: "完成任务包校验", createdAt: "2026-07-30T12:00:00Z", updatedAt: "2026-07-30T12:00:00Z", sourceAgent: "codex", targetAgent: "claude", sourceSessionId: "session-001", projectPath: "C:\\work\\agentpack", gitRepository: "C:\\work\\agentpack", gitHead: "abc123", includedSections: ["goal.json"], contentHashes: {}, warnings: [] },
  goal: { originalGoal: "实现真实任务包校验" }, state: { currentState: "代码已生成" }, decisions: [{ decision: "使用 SHA-256" }], failedAttempts: [],
  constraints: { constraints: ["不上传数据"] }, capabilities: { requiredTools: ["git"] }, nextActions: { actions: [{ priority: 1, action: "运行测试" }] },
  provenance: { sourceAgent: "codex" }, securityFindings: [], resumePrompt: "你正在接手一个由其他 AI Agent 中断的任务。",
};

export const settingsFixture: AppSettings = {
  sessionPaths: ["C:\\Users\\test\\.codex\\sessions"], packageOutputPath: "C:\\packs", autoScan: false, readGitState: true,
  collectCommandLogs: true, includeUntrackedFiles: false, securityScan: true, theme: "dark", databasePath: "C:\\data\\agentpack.sqlite3", logLevel: "info",
  agentInstallPaths: {}, defaultWorkingDirectory: "C:\\work", defaultContextBudget: 32000, compressionStrategy: "balanced", autoWatch: true,
  saveModelThoughts: false, terminalProgram: "cmd.exe", codexCommand: "codex", claudeCommand: "claude",
  recentMessageLimit: 24, autoScanIntervalSeconds: 5, toolOutputMaxLength: 12000, backupDirectory: "C:\\backups",
  healthWarningRatio: 0.72, healthCriticalRatio: 0.9, runInBackground: false,
};
