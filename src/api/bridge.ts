import { invoke } from "@tauri-apps/api/core";
import type {
  AppSettings,
  DashboardStats,
  PackageDetail,
  PackageDraft,
  PackageSummary,
  SessionDetail,
  SessionSummary,
  ValidationReport,
  UnifiedProjectSummary,
  UnifiedProjectDetail,
  CreateProjectInput,
  ConversationNode,
  ConversationBranch,
  ContextCompileOptions,
  CompiledContext,
  ContextSnapshot,
  ContextSnapshotDiff,
  ContinuationRecord,
  ConfigurationInventory,
  ContinuationPollResult,
  CodexCapabilityReport,
  WatchPollResult,
  DatabaseHealth,
  DatabaseBackupRecord,
  CodexProfile,
  BranchComparison,
  GlobalSearchResult,
  DiagnosticsReport,
  DiagnosticPathStatus,
} from "../types/models";

const browserDefaults: AppSettings = {
  sessionPaths: [],
  packageOutputPath: "",
  agentInstallPaths: {},
  defaultWorkingDirectory: "",
  defaultContextBudget: 32000,
  compressionStrategy: "balanced",
  autoScan: false,
  autoWatch: true,
  readGitState: true,
  collectCommandLogs: true,
  includeUntrackedFiles: false,
  saveModelThoughts: false,
  securityScan: true,
  theme: "dark",
  databasePath: "仅在 Tauri 桌面运行时中可用",
  logLevel: "info",
  terminalProgram: "cmd.exe",
  codexCommand: "codex",
  claudeCommand: "claude",
  recentMessageLimit: 24,
  autoScanIntervalSeconds: 5,
  toolOutputMaxLength: 12000,
  backupDirectory: "",
  healthWarningRatio: 0.72,
  healthCriticalRatio: 0.9,
  runInBackground: false,
};

function isTauri(): boolean {
  return typeof window !== "undefined" && Boolean(window.__TAURI_INTERNALS__);
}

function desktopOnlyError(): Error {
  return new Error("此操作需要在 Continuum 桌面客户端中运行");
}

async function desktopInvoke<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (!isTauri()) throw desktopOnlyError();
  return invoke<T>(command, args);
}

export const appApi = {
  async dashboard(): Promise<DashboardStats> {
    if (!isTauri()) {
      return {
        sessionCount: 0,
        packageCount: 0,
        importedPackageCount: 0,
        detectedAgents: [],
        lastScanAt: null,
        recentPackages: [],
        databasePath: browserDefaults.databasePath,
      };
    }
    return desktopInvoke("get_dashboard");
  },
  sessions: () => desktopInvoke<SessionSummary[]>("list_sessions"),
  session: (id: string) => desktopInvoke<SessionDetail>("get_session", { id }),
  scan: () => desktopInvoke<SessionSummary[]>("scan_sessions"),
  detectCodex: (force = false) =>
    desktopInvoke<CodexCapabilityReport>("detect_codex_capabilities", {
      force,
    }),
  probeCodexAppServer: () => desktopInvoke<string>("probe_codex_app_server"),
  pollSessionChanges: () =>
    desktopInvoke<WatchPollResult>("poll_session_changes"),
  reindexSession: (sessionId: string) =>
    desktopInvoke<SessionDetail>("reindex_session", { sessionId }),
  packages: () => desktopInvoke<PackageSummary[]>("list_packages"),
  package: (id: string) => desktopInvoke<PackageDetail>("get_package", { id }),
  packageDraft: (sessionId: string) =>
    desktopInvoke<PackageDraft>("prepare_package_draft", { sessionId }),
  createPackage: (draft: PackageDraft) =>
    desktopInvoke<PackageSummary>("create_package", { draft }),
  importPackage: (path: string) =>
    desktopInvoke<PackageSummary>("import_package", { path }),
  exportZip: (id: string, destination?: string) =>
    desktopInvoke<string>("export_package_zip", { id, destination }),
  exportFolder: (id: string, destination: string) =>
    desktopInvoke<string>("export_package_folder", { id, destination }),
  validatePackage: (id: string) =>
    desktopInvoke<ValidationReport>("validate_package", { id }),
  deletePackage: (id: string) => desktopInvoke<void>("delete_package", { id }),
  markResumed: (id: string) =>
    desktopInvoke<void>("mark_package_resumed", { id }),
  settings: async () =>
    isTauri() ? desktopInvoke<AppSettings>("get_settings") : browserDefaults,
  saveSettings: (settings: AppSettings) =>
    desktopInvoke<AppSettings>("save_settings", { settings }),
  projects: () => desktopInvoke<UnifiedProjectSummary[]>("list_projects"),
  createProject: (input: CreateProjectInput) =>
    desktopInvoke<UnifiedProjectDetail>("create_project", { input }),
  project: (id: string) =>
    desktopInvoke<UnifiedProjectDetail>("get_project", { id }),
  archiveProject: (id: string) =>
    desktopInvoke<void>("archive_project", { id }),
  restoreProject: (id: string) =>
    desktopInvoke<void>("restore_project", { id }),
  renameProject: (id: string, name: string) =>
    desktopInvoke<void>("rename_project", { id, name }),
  relocateProject: (id: string, projectPath: string) =>
    desktopInvoke<void>("relocate_project", { id, projectPath }),
  deleteProjectRecord: (id: string) =>
    desktopInvoke<void>("delete_project_record", { id }),
  unbindSession: (projectId: string, sessionId: string) =>
    desktopInvoke<void>("unbind_project_session", { projectId, sessionId }),
  rebindSession: (sessionId: string, projectId: string, branchId: string) =>
    desktopInvoke<UnifiedProjectDetail>("rebind_project_session", {
      sessionId,
      projectId,
      branchId,
    }),
  suggestProjects: (sessionId: string) =>
    desktopInvoke<UnifiedProjectSummary[]>("suggest_projects_for_session", {
      sessionId,
    }),
  databaseHealth: () => desktopInvoke<DatabaseHealth>("check_database"),
  createDatabaseBackup: (reason?: string) =>
    desktopInvoke<DatabaseBackupRecord>("create_database_backup", { reason }),
  databaseBackups: () =>
    desktopInvoke<DatabaseBackupRecord[]>("list_database_backups"),
  restoreDatabaseBackup: (backupPath: string) =>
    desktopInvoke<DatabaseHealth>("restore_database_backup", { backupPath }),
  diagnostics: (forceCodex = false) =>
    desktopInvoke<DiagnosticsReport>("get_diagnostics", { forceCodex }),
  diagnosticsReport: () => desktopInvoke<string>("copy_diagnostics_report"),
  exportDiagnostics: (path: string) =>
    desktopInvoke<string>("export_diagnostics_report", { path }),
  validateSettingsPaths: (settings: AppSettings) =>
    desktopInvoke<DiagnosticPathStatus[]>("validate_settings_paths", {
      settings,
    }),
  bindSessions: (projectId: string, sessionIds: string[], branchId?: string) =>
    desktopInvoke<UnifiedProjectDetail>("bind_sessions_to_project", {
      projectId,
      sessionIds,
      branchId,
    }),
  timeline: (projectId: string, branchId: string) =>
    desktopInvoke<ConversationNode[]>("get_unified_timeline", {
      projectId,
      branchId,
    }),
  addNote: (
    projectId: string,
    branchId: string,
    content: string,
    parentNodeId?: string,
  ) =>
    desktopInvoke<ConversationNode>("add_user_note", {
      projectId,
      branchId,
      content,
      parentNodeId,
    }),
  createBranch: (projectId: string, fromNodeId: string, name: string) =>
    desktopInvoke<ConversationBranch>("create_conversation_branch", {
      projectId,
      fromNodeId,
      name,
    }),
  updateNode: (nodeId: string, status: string, importance: number) =>
    desktopInvoke<ConversationNode>("update_conversation_node", {
      nodeId,
      status,
      importance,
    }),
  renameBranch: (branchId: string, name: string) =>
    desktopInvoke<void>("rename_conversation_branch", { branchId, name }),
  archiveBranch: (branchId: string) =>
    desktopInvoke<void>("archive_conversation_branch", { branchId }),
  restoreBranch: (branchId: string) =>
    desktopInvoke<void>("restore_conversation_branch", { branchId }),
  switchBranch: (projectId: string, branchId: string) =>
    desktopInvoke<void>("switch_conversation_branch", { projectId, branchId }),
  deleteBranch: (branchId: string) =>
    desktopInvoke<void>("delete_conversation_branch", { branchId }),
  compareBranches: (sourceBranchId: string, targetBranchId: string) =>
    desktopInvoke<BranchComparison>("compare_conversation_branches", {
      sourceBranchId,
      targetBranchId,
    }),
  mergeBranchItems: (
    sourceBranchId: string,
    targetBranchId: string,
    nodeIds: string[],
  ) =>
    desktopInvoke<ConversationNode>("merge_branch_context_items", {
      sourceBranchId,
      targetBranchId,
      nodeIds,
    }),
  globalSearch: (query: string, limit = 80) =>
    desktopInvoke<GlobalSearchResult[]>("global_search", { query, limit }),
  syncProject: (projectId: string) =>
    desktopInvoke<number>("sync_project_sessions", { projectId }),
  compileContext: (options: ContextCompileOptions) =>
    desktopInvoke<CompiledContext>("compile_context", { options }),
  saveSnapshot: (options: ContextCompileOptions) =>
    desktopInvoke<ContextSnapshot>("save_context_snapshot", { options }),
  snapshots: (projectId: string) =>
    desktopInvoke<ContextSnapshot[]>("list_context_snapshots", { projectId }),
  diffSnapshots: (fromSnapshotId: string, toSnapshotId: string) =>
    desktopInvoke<ContextSnapshotDiff>("diff_context_snapshots", {
      fromSnapshotId,
      toSnapshotId,
    }),
  setContextItemOverride: (input: {
    projectId: string;
    branchId: string | null;
    sourceNodeId: string | null;
    contentHash: string;
    action?: string | null;
    priority?: number | null;
    pinned?: boolean | null;
    stale?: boolean | null;
    incorrect?: boolean | null;
    permanent: boolean;
  }) => desktopInvoke<void>("set_context_item_override", input),
  createContinuation: (options: ContextCompileOptions, launch: boolean) =>
    desktopInvoke<ContinuationRecord>("create_continuation", {
      options,
      launch,
    }),
  launchContinuation: (continuationId: string) =>
    desktopInvoke<ContinuationRecord>("launch_continuation", {
      continuationId,
    }),
  continuations: (projectId: string) =>
    desktopInvoke<ContinuationRecord[]>("list_continuations", { projectId }),
  bindContinuation: (continuationId: string, sessionId: string) =>
    desktopInvoke<void>("bind_continuation_session", {
      continuationId,
      sessionId,
    }),
  cancelContinuation: (continuationId: string) =>
    desktopInvoke<ContinuationRecord>("cancel_continuation", {
      continuationId,
    }),
  retryContinuation: (continuationId: string) =>
    desktopInvoke<ContinuationRecord>("retry_continuation", { continuationId }),
  recoverContinuations: () =>
    desktopInvoke<ContinuationRecord[]>("recover_continuations"),
  cleanupContinuationContext: (continuationId: string) =>
    desktopInvoke<ContinuationRecord>("cleanup_continuation_context", {
      continuationId,
    }),
  pollContinuation: (continuationId: string) =>
    desktopInvoke<ContinuationPollResult>("poll_continuation", {
      continuationId,
    }),
  launchSourceSession: (sessionId: string, operation: "resume" | "fork") =>
    desktopInvoke<number>("launch_source_session", { sessionId, operation }),
  scanConfigurations: (projectId?: string) =>
    desktopInvoke<ConfigurationInventory>("scan_configurations", { projectId }),
  bindConfiguration: (
    projectId: string,
    kind: "skill" | "mcp",
    itemId: string,
    bound: boolean,
  ) =>
    desktopInvoke<void>("bind_configuration", {
      projectId,
      kind,
      itemId,
      bound,
    }),
  codexProfiles: (projectId?: string) =>
    desktopInvoke<CodexProfile[]>("list_codex_profiles", { projectId }),
  createCodexProfile: (projectId?: string, branchId?: string) =>
    desktopInvoke<CodexProfile>("create_default_codex_profile", {
      projectId,
      branchId,
    }),
  saveCodexProfile: (profile: CodexProfile) =>
    desktopInvoke<CodexProfile>("save_codex_profile", { profile }),
  duplicateCodexProfile: (id: string, name: string) =>
    desktopInvoke<CodexProfile>("duplicate_codex_profile", { id, name }),
  deleteCodexProfile: (id: string) =>
    desktopInvoke<void>("delete_codex_profile", { id }),
  setProjectCodexProfile: (projectId: string, profileId: string) =>
    desktopInvoke<void>("set_project_codex_profile", { projectId, profileId }),
  setBranchCodexProfile: (
    projectId: string,
    branchId: string,
    profileId: string,
  ) =>
    desktopInvoke<void>("set_branch_codex_profile", {
      projectId,
      branchId,
      profileId,
    }),
  exportCodexProfile: (id: string, path: string) =>
    desktopInvoke<string>("export_codex_profile", { id, path }),
  importCodexProfile: (path: string) =>
    desktopInvoke<CodexProfile>("import_codex_profile", { path }),
};
