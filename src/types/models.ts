export type AgentKind =
  "codex" | "claude" | "gemini" | "opencode" | "cursor" | "copilot";

export interface DashboardStats {
  sessionCount: number;
  packageCount: number;
  importedPackageCount: number;
  detectedAgents: AgentKind[];
  lastScanAt: string | null;
  recentPackages: PackageSummary[];
  databasePath: string;
}

export interface SessionSummary {
  id: string;
  title: string;
  agent: AgentKind;
  createdAt: string;
  updatedAt: string;
  workingDirectory: string | null;
  gitRepository: string | null;
  messageCount: number;
  toolCallCount: number;
  hasFileChanges: boolean;
  canPackage: boolean;
  sourcePath: string;
  parseWarning: string | null;
}

export interface CodexCapabilityReport {
  capabilitySchemaVersion: number;
  installed: boolean;
  executablePath: string | null;
  version: string | null;
  helpHash: string | null;
  supportsResume: boolean;
  supportsFork: boolean;
  supportsCd: boolean;
  supportsModel: boolean;
  supportsProfile: boolean;
  supportsSandbox: boolean;
  supportsApproval: boolean;
  supportsAppServer: boolean;
  sessionPaths: string[];
  checkedAt: string;
  error: string | null;
}

export interface CodexProfile {
  id: string;
  projectId: string | null;
  branchId: string | null;
  name: string;
  executablePath: string;
  model: string | null;
  workingDirectory: string;
  approvalMode: "untrusted" | "on-request" | "never";
  sandboxMode: "read-only" | "workspace-write" | "danger-full-access";
  launchArguments: string[];
  contextBudget: number;
  recentMessageLimit: number;
  includeGitStatus: boolean;
  includeGitDiff: boolean;
  includeTests: boolean;
  includeFailedAttempts: boolean;
  includeSkills: boolean;
  includeMcp: boolean;
  launchPromptTemplate: string;
  createdAt: string;
  updatedAt: string;
}

export interface WatchPollResult {
  scannedFiles: number;
  newSessions: number;
  updatedSessions: number;
  insertedNodes: number;
  parseErrors: number;
}

export interface DatabaseHealth {
  path: string;
  schemaVersion: number;
  integrity: string;
  sizeBytes: number;
  orphanNodes: number;
  invalidBindings: number;
  checkedAt: string;
}

export interface DatabaseBackupRecord {
  id: string;
  path: string;
  reason: string;
  schemaVersion: number;
  sizeBytes: number;
  sha256: string;
  createdAt: string;
  restoredAt: string | null;
}

export interface SessionMessage {
  id: string;
  role: "user" | "assistant" | "system" | "tool" | "unknown";
  content: string;
  timestamp: string | null;
}

export interface ToolCall {
  id: string;
  name: string;
  arguments: string;
  status: "success" | "failed" | "unknown";
  output: string | null;
  timestamp: string | null;
}

export interface GitState {
  isRepository: boolean;
  repositoryPath: string | null;
  branch: string | null;
  head: string | null;
  modified: string[];
  staged: string[];
  untracked: string[];
  workingTreeDiff: string;
  stagedDiff: string;
  error: string | null;
}

export interface SessionDetail extends SessionSummary {
  goalSummary: string;
  messages: SessionMessage[];
  toolCalls: ToolCall[];
  commands: string[];
  changedFiles: string[];
  failedSteps: string[];
  gitState: GitState | null;
  rawData: unknown[];
}

export interface PackageSummary {
  id: string;
  title: string;
  sourceAgent: AgentKind;
  targetAgent: AgentKind;
  createdAt: string;
  projectPath: string | null;
  packagePath: string;
  schemaVersion: string;
  integrity: "valid" | "warning" | "invalid" | "unchecked";
  hasPatch: boolean;
  securityWarningCount: number;
  imported: boolean;
  resumed: boolean;
}

export interface PackageManifest {
  schemaVersion: "1.0.0-alpha.1";
  packageId: string;
  title: string;
  createdAt: string;
  updatedAt: string;
  sourceAgent: AgentKind;
  targetAgent: AgentKind;
  sourceSessionId: string;
  projectPath: string | null;
  gitRepository: string | null;
  gitHead: string | null;
  includedSections: string[];
  contentHashes: Record<string, string>;
  warnings: string[];
}

export interface SecurityFinding {
  findingType: string;
  sourceFile: string;
  fieldPath: string;
  severity: "low" | "medium" | "high";
}

export interface PackageDetail extends PackageSummary {
  manifest: PackageManifest;
  goal: Record<string, unknown>;
  state: Record<string, unknown>;
  decisions: Array<Record<string, unknown>>;
  failedAttempts: Array<Record<string, unknown>>;
  constraints: Record<string, unknown>;
  capabilities: Record<string, unknown>;
  nextActions: Record<string, unknown>;
  provenance: Record<string, unknown>;
  securityFindings: SecurityFinding[];
  resumePrompt: string;
}

export interface PackageDraft {
  sourceSessionId: string;
  title: string;
  originalGoal: string;
  currentState: string;
  completedWork: string;
  remainingWork: string;
  nextActions: string;
  decisions: string;
  knownIssues: string;
  failedAttempts: string;
  constraints: string;
  requiredTools: string;
  targetAgent: AgentKind;
  includeGit: boolean;
  includePatch: boolean;
  includeUntracked: boolean;
  includeTests: boolean;
  includeCommandLog: boolean;
}

export interface AppSettings {
  sessionPaths: string[];
  packageOutputPath: string;
  agentInstallPaths: Record<string, string>;
  defaultWorkingDirectory: string;
  defaultContextBudget: number;
  compressionStrategy: "balanced" | "conservative" | "aggressive";
  autoScan: boolean;
  autoWatch: boolean;
  readGitState: boolean;
  collectCommandLogs: boolean;
  includeUntrackedFiles: boolean;
  saveModelThoughts: boolean;
  securityScan: boolean;
  theme: "dark" | "system";
  databasePath: string;
  logLevel: "error" | "warn" | "info" | "debug";
  terminalProgram: string;
  codexCommand: string;
  claudeCommand: string;
  recentMessageLimit: number;
  autoScanIntervalSeconds: number;
  toolOutputMaxLength: number;
  backupDirectory: string;
  healthWarningRatio: number;
  healthCriticalRatio: number;
  runInBackground: boolean;
}

export type ContextHealthLevel =
  | "healthy"
  | "growing"
  | "compression_recommended"
  | "fresh_continuation_recommended"
  | "critical";

export interface ContextHealth {
  level: ContextHealthLevel;
  messageCount: number;
  estimatedTokens: number;
  duplicateRatio: number;
  toolLogRatio: number;
  staleRatio: number;
  incorrectRatio: number;
  conflictCount: number;
  uncompressedLogCount: number;
  contextBudget: number;
  thresholdRatio: number;
  lastSnapshotAt: string | null;
  lastFreshContinuationAt: string | null;
  currentSessionDurationSeconds: number | null;
  reasons: string[];
}

export interface UnifiedProjectSummary {
  id: string;
  name: string;
  projectPath: string;
  gitRepository: string | null;
  goal: string;
  currentTask: string;
  currentBranchId: string;
  currentBranchName: string;
  defaultAgent: AgentKind;
  defaultModel: string;
  sessionCount: number;
  updatedAt: string;
  archived: boolean;
  pathExists: boolean;
  health: ContextHealth;
}

export interface ConversationBranch {
  id: string;
  projectId: string;
  name: string;
  parentBranchId: string | null;
  forkNodeId: string | null;
  status: "active" | "archived" | "merged" | "abandoned";
  createdAt: string;
  updatedAt: string;
  nodeCount: number;
}

export interface BranchComparison {
  sourceBranchId: string;
  targetBranchId: string;
  sourceOnly: Record<string, string[]>;
  targetOnly: Record<string, string[]>;
}

export interface GlobalSearchResult {
  kind:
    | "project"
    | "branch"
    | "session"
    | "message"
    | "decision"
    | "error"
    | "file"
    | "command"
    | "test"
    | "skill"
    | "mcp"
    | string;
  id: string;
  title: string;
  excerpt: string;
  projectId: string | null;
  branchId: string | null;
  sessionId: string | null;
  path: string | null;
  createdAt: string | null;
}

export interface DiagnosticPathStatus {
  path: string;
  readable: boolean;
  writable: boolean;
  exists: boolean;
}

export interface DiagnosticsReport {
  continuumVersion: string;
  osVersion: string;
  webviewVersion: string | null;
  nodeVersion: string | null;
  rustVersion: string | null;
  codex: CodexCapabilityReport;
  sessionPaths: DiagnosticPathStatus[];
  database: DatabaseHealth;
  watcherEnabled: boolean;
  watcherIntervalSeconds: number;
  recentScan: string | null;
  recentContinuation: string | null;
  recentErrors: string[];
  logDirectory: string;
  dataDirectory: string;
  backupCount: number;
  generatedAt: string;
}

export type ConversationNodeType =
  | "message"
  | "tool_call"
  | "file_change"
  | "session_switch"
  | "summary"
  | "decision"
  | "constraint"
  | "todo"
  | "user_note"
  | "error"
  | "git_commit"
  | "branch_point";

export interface ConversationNode {
  id: string;
  projectId: string;
  parentNodeId: string | null;
  branchId: string;
  sourceAgent: AgentKind | null;
  sourceSessionId: string | null;
  nodeType: ConversationNodeType;
  content: string;
  createdAt: string;
  importance: number;
  status: "active" | "completed" | "stale" | "incorrect" | "excluded";
  metadata: Record<string, unknown>;
}

export interface BoundSourceSession {
  id: string;
  agent: AgentKind;
  title: string;
  sourcePath: string;
  branchId: string;
  messageCount: number;
  lastSyncedAt: string;
  continuationId: string | null;
}

export interface UnifiedProjectDetail extends UnifiedProjectSummary {
  constraints: string[];
  branches: ConversationBranch[];
  sessions: BoundSourceSession[];
  activeFiles: string[];
  decisions: ConversationNode[];
  todos: ConversationNode[];
  gitState: GitState | null;
}

export interface CreateProjectInput {
  name: string;
  projectPath: string;
  goal: string;
  constraints: string[];
  defaultAgent: AgentKind;
  defaultModel: string;
}

export interface ContextCompileOptions {
  projectId: string;
  branchId: string;
  sourceNodeId: string | null;
  targetAgent: AgentKind;
  targetModel: string;
  tokenBudget: number;
  recentRounds: number;
  includeToolLogs: boolean;
  includeGitDiff: boolean;
  includeFailedAttempts: boolean;
  includeSkills: boolean;
  includeMcp: boolean;
}

export type ContextItemAction =
  "keep" | "compress" | "retrieve_only" | "exclude";

export interface ContextItem {
  id: string;
  sourceNodeId: string | null;
  category: string;
  action: ContextItemAction;
  reason: string;
  estimatedTokens: number;
  content: string;
  pinned: boolean;
  priority: number;
  stale: boolean;
  incorrect: boolean;
  permanent: boolean;
  contentHash: string;
}

export interface CompiledContext {
  projectId: string;
  branchId: string;
  targetAgent: AgentKind;
  targetModel: string;
  tokenBudget: number;
  estimatedTokens: number;
  originalEstimatedTokens: number;
  contentHash: string;
  generatedAt: string;
  systemContext: string;
  compiledText: string;
  items: ContextItem[];
  conflicts: string[];
  health: ContextHealth;
}

export interface ContextSnapshotDiff {
  fromSnapshotId: string;
  toSnapshotId: string;
  added: ContextItem[];
  removed: ContextItem[];
  changed: ContextItem[];
  tokenDelta: number;
}

export interface ContextSnapshot extends CompiledContext {
  id: string;
  sourceNodeId: string | null;
}

export interface ContinuationRecord {
  id: string;
  projectId: string;
  branchId: string;
  sourceNodeId: string | null;
  snapshotId: string;
  targetAgent: AgentKind;
  targetModel: string;
  mode: "native" | "context" | "export_only";
  status:
    | "idle"
    | "compiling_context"
    | "writing_context"
    | "preparing_launch"
    | "launching"
    | "waiting_for_session"
    | "candidate_sessions_found"
    | "binding"
    | "listening"
    | "completed"
    | "launch_failed"
    | "detection_timeout"
    | "manual_binding_required"
    | "cancelled";
  bootstrapFile: string;
  launchCommand: string;
  targetSessionId: string | null;
  createdAt: string;
  warning: string | null;
  processId: number | null;
  workingDirectory: string;
  contextHash: string;
  marker: string;
  startedAt: string;
  detectedAt: string | null;
  listening: boolean;
}

export interface ContinuationPollResult {
  continuation: ContinuationRecord;
  candidates: SessionSummary[];
  insertedNodes: number;
}

export interface UnifiedSkill {
  id: string;
  name: string;
  description: string;
  sourcePlatform: string;
  sourcePath: string;
  compatibleAgents: AgentKind[];
  requiredTools: string[];
  instructions: string;
  installationState:
    | "available"
    | "convertible"
    | "manual"
    | "incompatible"
    | "missing_dependency";
  bound: boolean;
}

export interface McpServerInfo {
  id: string;
  name: string;
  sourceAgent: AgentKind;
  command: string | null;
  transport: string;
  compatibleAgents: AgentKind[];
  bound: boolean;
}

export interface ConfigurationInventory {
  skills: UnifiedSkill[];
  mcpServers: McpServerInfo[];
  customInstructions: Array<{
    id: string;
    name: string;
    path: string;
    sourceAgent: AgentKind;
  }>;
}

export interface ValidationIssue {
  code: string;
  message: string;
  path: string | null;
  severity: "warning" | "error";
}

export interface ValidationReport {
  valid: boolean;
  checkedAt: string;
  issues: ValidationIssue[];
}
