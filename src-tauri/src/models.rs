use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AgentKind {
    Codex,
    Claude,
    Gemini,
    Opencode,
    Cursor,
    Copilot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AppSettings {
    pub session_paths: Vec<String>,
    pub package_output_path: String,
    pub auto_scan: bool,
    pub read_git_state: bool,
    pub collect_command_logs: bool,
    pub include_untracked_files: bool,
    pub security_scan: bool,
    pub theme: String,
    pub database_path: String,
    pub log_level: String,
    pub agent_install_paths: BTreeMap<String, String>,
    pub default_working_directory: String,
    pub default_context_budget: usize,
    pub compression_strategy: String,
    pub auto_watch: bool,
    pub save_model_thoughts: bool,
    pub terminal_program: String,
    pub codex_command: String,
    pub claude_command: String,
    pub recent_message_limit: usize,
    pub auto_scan_interval_seconds: u64,
    pub tool_output_max_length: usize,
    pub backup_directory: String,
    pub health_warning_ratio: f64,
    pub health_critical_ratio: f64,
    pub run_in_background: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            session_paths: vec![],
            package_output_path: String::new(),
            auto_scan: false,
            read_git_state: true,
            collect_command_logs: true,
            include_untracked_files: false,
            security_scan: true,
            theme: "dark".into(),
            database_path: String::new(),
            log_level: "info".into(),
            agent_install_paths: BTreeMap::new(),
            default_working_directory: String::new(),
            default_context_budget: 32_000,
            compression_strategy: "balanced".into(),
            auto_watch: true,
            save_model_thoughts: false,
            terminal_program: "cmd.exe".into(),
            codex_command: "codex".into(),
            claude_command: "claude".into(),
            recent_message_limit: 24,
            auto_scan_interval_seconds: 15,
            tool_output_max_length: 12_000,
            backup_directory: String::new(),
            health_warning_ratio: 0.72,
            health_critical_ratio: 0.90,
            run_in_background: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageSummary {
    pub id: String,
    pub title: String,
    pub source_agent: AgentKind,
    pub target_agent: AgentKind,
    pub created_at: String,
    pub project_path: Option<String>,
    pub package_path: String,
    pub schema_version: String,
    pub integrity: String,
    pub has_patch: bool,
    pub security_warning_count: usize,
    pub imported: bool,
    pub resumed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardStats {
    pub session_count: usize,
    pub package_count: usize,
    pub imported_package_count: usize,
    pub detected_agents: Vec<AgentKind>,
    pub last_scan_at: Option<String>,
    pub recent_packages: Vec<PackageSummary>,
    pub database_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub id: String,
    pub title: String,
    pub agent: AgentKind,
    pub created_at: String,
    pub updated_at: String,
    pub working_directory: Option<String>,
    pub git_repository: Option<String>,
    pub message_count: usize,
    pub tool_call_count: usize,
    pub has_file_changes: bool,
    pub can_package: bool,
    pub source_path: String,
    pub parse_warning: Option<String>,
    pub client_kind: String,
    pub bound_project_id: Option<String>,
    pub bound_project_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Tool,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMessage {
    pub id: String,
    pub role: MessageRole,
    pub content: String,
    pub timestamp: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolStatus {
    Success,
    Failed,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
    pub status: ToolStatus,
    pub output: Option<String>,
    pub timestamp: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitState {
    pub is_repository: bool,
    pub repository_path: Option<String>,
    pub branch: Option<String>,
    pub head: Option<String>,
    pub modified: Vec<String>,
    pub staged: Vec<String>,
    pub untracked: Vec<String>,
    pub working_tree_diff: String,
    pub staged_diff: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDetail {
    #[serde(flatten)]
    pub summary: SessionSummary,
    pub goal_summary: String,
    pub messages: Vec<SessionMessage>,
    pub tool_calls: Vec<ToolCall>,
    pub commands: Vec<String>,
    pub changed_files: Vec<String>,
    pub failed_steps: Vec<String>,
    pub git_state: Option<GitState>,
    pub raw_data: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageDraft {
    pub source_session_id: String,
    pub title: String,
    pub original_goal: String,
    pub current_state: String,
    pub completed_work: String,
    pub remaining_work: String,
    pub next_actions: String,
    pub decisions: String,
    pub known_issues: String,
    pub failed_attempts: String,
    pub constraints: String,
    pub required_tools: String,
    pub target_agent: AgentKind,
    pub include_git: bool,
    pub include_patch: bool,
    pub include_untracked: bool,
    pub include_tests: bool,
    pub include_command_log: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageManifest {
    pub schema_version: String,
    pub package_id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub source_agent: AgentKind,
    pub target_agent: AgentKind,
    pub source_session_id: String,
    pub project_path: Option<String>,
    pub git_repository: Option<String>,
    pub git_head: Option<String>,
    pub included_sections: Vec<String>,
    pub content_hashes: BTreeMap<String, String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityFinding {
    pub finding_type: String,
    pub source_file: String,
    pub field_path: String,
    pub severity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageDetail {
    #[serde(flatten)]
    pub summary: PackageSummary,
    pub manifest: PackageManifest,
    pub goal: Value,
    pub state: Value,
    pub decisions: Vec<Value>,
    pub failed_attempts: Vec<Value>,
    pub constraints: Value,
    pub capabilities: Value,
    pub next_actions: Value,
    pub provenance: Value,
    pub security_findings: Vec<SecurityFinding>,
    pub resume_prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationIssue {
    pub code: String,
    pub message: String,
    pub path: Option<String>,
    pub severity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationReport {
    pub valid: bool,
    pub checked_at: String,
    pub issues: Vec<ValidationIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextHealthLevel {
    Healthy,
    Growing,
    #[serde(alias = "compress")]
    CompressionRecommended,
    #[serde(alias = "new_session")]
    FreshContinuationRecommended,
    #[serde(alias = "high_risk")]
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextHealth {
    pub level: ContextHealthLevel,
    pub message_count: usize,
    pub estimated_tokens: usize,
    pub duplicate_ratio: f64,
    pub tool_log_ratio: f64,
    pub stale_ratio: f64,
    #[serde(default)]
    pub incorrect_ratio: f64,
    #[serde(default)]
    pub conflict_count: usize,
    #[serde(default)]
    pub uncompressed_log_count: usize,
    pub context_budget: usize,
    pub threshold_ratio: f64,
    pub last_snapshot_at: Option<String>,
    #[serde(default)]
    pub last_fresh_continuation_at: Option<String>,
    #[serde(default)]
    pub current_session_duration_seconds: Option<i64>,
    #[serde(default)]
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnifiedProjectSummary {
    pub id: String,
    pub name: String,
    pub project_path: String,
    pub git_repository: Option<String>,
    pub goal: String,
    pub current_task: String,
    pub current_branch_id: String,
    pub current_branch_name: String,
    pub default_agent: AgentKind,
    pub default_model: String,
    pub session_count: usize,
    pub updated_at: String,
    pub archived: bool,
    pub path_exists: bool,
    pub health: ContextHealth,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationBranch {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub parent_branch_id: Option<String>,
    pub fork_node_id: Option<String>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub node_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationNode {
    pub id: String,
    pub project_id: String,
    pub parent_node_id: Option<String>,
    pub branch_id: String,
    pub source_agent: Option<AgentKind>,
    pub source_session_id: Option<String>,
    pub node_type: String,
    pub content: String,
    pub created_at: String,
    pub importance: i32,
    pub status: String,
    pub metadata: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoundSourceSession {
    pub id: String,
    pub agent: AgentKind,
    pub title: String,
    pub source_path: String,
    pub branch_id: String,
    pub message_count: usize,
    pub last_synced_at: String,
    pub continuation_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnifiedProjectDetail {
    #[serde(flatten)]
    pub summary: UnifiedProjectSummary,
    pub constraints: Vec<String>,
    pub branches: Vec<ConversationBranch>,
    pub sessions: Vec<BoundSourceSession>,
    pub active_files: Vec<String>,
    pub decisions: Vec<ConversationNode>,
    pub todos: Vec<ConversationNode>,
    pub git_state: Option<GitState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectInput {
    pub name: String,
    pub project_path: String,
    pub goal: String,
    pub constraints: Vec<String>,
    pub default_agent: AgentKind,
    pub default_model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextCompileOptions {
    pub project_id: String,
    pub branch_id: String,
    pub source_node_id: Option<String>,
    pub target_agent: AgentKind,
    pub target_model: String,
    pub token_budget: usize,
    pub recent_rounds: usize,
    pub include_tool_logs: bool,
    pub include_git_diff: bool,
    pub include_failed_attempts: bool,
    pub include_skills: bool,
    pub include_mcp: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextItem {
    pub id: String,
    pub source_node_id: Option<String>,
    pub category: String,
    pub action: String,
    pub reason: String,
    pub estimated_tokens: usize,
    pub content: String,
    pub pinned: bool,
    #[serde(default = "default_context_priority")]
    pub priority: i32,
    #[serde(default)]
    pub stale: bool,
    #[serde(default)]
    pub incorrect: bool,
    #[serde(default)]
    pub permanent: bool,
    #[serde(default)]
    pub content_hash: String,
}

fn default_context_priority() -> i32 {
    50
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledContext {
    pub project_id: String,
    pub branch_id: String,
    pub target_agent: AgentKind,
    pub target_model: String,
    pub token_budget: usize,
    pub estimated_tokens: usize,
    #[serde(default)]
    pub original_estimated_tokens: usize,
    #[serde(default)]
    pub content_hash: String,
    pub generated_at: String,
    pub system_context: String,
    pub compiled_text: String,
    pub items: Vec<ContextItem>,
    pub conflicts: Vec<String>,
    pub health: ContextHealth,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextSnapshotDiff {
    pub from_snapshot_id: String,
    pub to_snapshot_id: String,
    pub added: Vec<ContextItem>,
    pub removed: Vec<ContextItem>,
    pub changed: Vec<ContextItem>,
    pub token_delta: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextSnapshot {
    pub id: String,
    pub source_node_id: Option<String>,
    #[serde(flatten)]
    pub compiled: CompiledContext,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContinuationMode {
    Native,
    Context,
    ExportOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuationRecord {
    pub id: String,
    pub project_id: String,
    pub branch_id: String,
    pub source_node_id: Option<String>,
    pub snapshot_id: String,
    pub target_agent: AgentKind,
    pub target_model: String,
    pub mode: ContinuationMode,
    pub status: String,
    pub bootstrap_file: String,
    pub launch_command: String,
    pub target_session_id: Option<String>,
    pub created_at: String,
    pub warning: Option<String>,
    pub process_id: Option<u32>,
    pub working_directory: String,
    pub context_hash: String,
    pub marker: String,
    pub started_at: String,
    pub detected_at: Option<String>,
    pub listening: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuationPollResult {
    pub continuation: ContinuationRecord,
    pub candidates: Vec<SessionSummary>,
    pub inserted_nodes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppServerClientRequest {
    pub id: String,
    pub continuation_id: String,
    pub project_id: String,
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub kind: String,
    pub reason: Option<String>,
    pub command: Option<String>,
    pub cwd: Option<String>,
    pub command_actions: Vec<Value>,
    pub grant_root: Option<String>,
    pub network_host: Option<String>,
    pub network_protocol: Option<String>,
    pub permissions: Option<Value>,
    pub server_name: Option<String>,
    pub message: Option<String>,
    pub mode: Option<String>,
    pub url: Option<String>,
    pub requested_schema: Option<Value>,
    pub metadata: Option<Value>,
    pub questions: Vec<Value>,
    pub auto_resolution_ms: Option<u64>,
    pub started_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnifiedSkill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source_platform: String,
    pub source_path: String,
    pub compatible_agents: Vec<AgentKind>,
    pub required_tools: Vec<String>,
    pub instructions: String,
    pub installation_state: String,
    pub bound: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerInfo {
    pub id: String,
    pub name: String,
    pub source_agent: AgentKind,
    pub command: Option<String>,
    pub transport: String,
    pub compatible_agents: Vec<AgentKind>,
    pub bound: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomInstructionInfo {
    pub id: String,
    pub name: String,
    pub path: String,
    pub source_agent: AgentKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationInventory {
    pub skills: Vec<UnifiedSkill>,
    pub mcp_servers: Vec<McpServerInfo>,
    pub custom_instructions: Vec<CustomInstructionInfo>,
}

#[allow(dead_code)] // Cross-agent adapter contract retained for P2 implementations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterCapabilities {
    pub native_resume: bool,
    pub native_fork: bool,
    pub fresh_context_launch: bool,
    pub session_watch: bool,
    pub skills_discovery: bool,
    pub mcp_discovery: bool,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContinuationStatus {
    Idle,
    CompilingContext,
    WritingContext,
    PreparingLaunch,
    Launching,
    WaitingForSession,
    CandidateSessionsFound,
    Binding,
    Listening,
    Completed,
    LaunchFailed,
    DetectionTimeout,
    ManualBindingRequired,
    Cancelled,
}

impl ContinuationStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::CompilingContext => "compiling_context",
            Self::WritingContext => "writing_context",
            Self::PreparingLaunch => "preparing_launch",
            Self::Launching => "launching",
            Self::WaitingForSession => "waiting_for_session",
            Self::CandidateSessionsFound => "candidate_sessions_found",
            Self::Binding => "binding",
            Self::Listening => "listening",
            Self::Completed => "completed",
            Self::LaunchFailed => "launch_failed",
            Self::DetectionTimeout => "detection_timeout",
            Self::ManualBindingRequired => "manual_binding_required",
            Self::Cancelled => "cancelled",
        }
    }
}

#[allow(dead_code)] // Public operation taxonomy reserved for the adapter UI.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CodexOperationKind {
    NativeResume,
    NativeFork,
    FreshContinuation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexCapabilityReport {
    #[serde(default)]
    pub capability_schema_version: u32,
    pub installed: bool,
    pub executable_path: Option<String>,
    pub version: Option<String>,
    pub help_hash: Option<String>,
    pub supports_resume: bool,
    pub supports_fork: bool,
    pub supports_cd: bool,
    pub supports_model: bool,
    pub supports_profile: bool,
    pub supports_sandbox: bool,
    pub supports_approval: bool,
    #[serde(default)]
    pub supports_app_server: bool,
    pub session_paths: Vec<String>,
    pub checked_at: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexProfile {
    pub id: String,
    pub project_id: Option<String>,
    pub branch_id: Option<String>,
    pub name: String,
    pub executable_path: String,
    pub model: Option<String>,
    pub working_directory: String,
    pub approval_mode: String,
    pub sandbox_mode: String,
    pub launch_arguments: Vec<String>,
    pub context_budget: usize,
    pub recent_message_limit: usize,
    pub include_git_status: bool,
    pub include_git_diff: bool,
    pub include_tests: bool,
    pub include_failed_attempts: bool,
    pub include_skills: bool,
    pub include_mcp: bool,
    pub launch_prompt_template: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceSessionCursor {
    pub session_id: String,
    pub session_file_path: String,
    pub last_imported_offset: u64,
    pub last_imported_line: usize,
    pub pending_fragment: String,
    pub file_hash: String,
    pub file_created_at: Option<String>,
    pub file_modified_at: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WatchPollResult {
    pub scanned_files: usize,
    pub new_sessions: usize,
    pub updated_sessions: usize,
    pub inserted_nodes: usize,
    pub parse_errors: usize,
}

#[allow(dead_code)] // Candidate persistence currently maps directly to SessionSummary.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuationCandidate {
    pub session_id: String,
    pub session_file_path: String,
    pub working_directory: String,
    pub created_at: String,
    pub modified_at: String,
    pub first_user_message: String,
    pub confidence: i32,
    pub validation: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchComparison {
    pub source_branch_id: String,
    pub target_branch_id: String,
    pub source_only: BTreeMap<String, Vec<String>>,
    pub target_only: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalSearchResult {
    pub kind: String,
    pub id: String,
    pub title: String,
    pub excerpt: String,
    pub project_id: Option<String>,
    pub branch_id: Option<String>,
    pub session_id: Option<String>,
    pub path: Option<String>,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticPathStatus {
    pub path: String,
    pub readable: bool,
    pub writable: bool,
    pub exists: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsReport {
    pub continuum_version: String,
    pub os_version: String,
    pub webview_version: Option<String>,
    pub node_version: Option<String>,
    pub rust_version: Option<String>,
    pub codex: CodexCapabilityReport,
    pub session_paths: Vec<DiagnosticPathStatus>,
    pub database: DatabaseHealth,
    pub watcher_enabled: bool,
    pub watcher_interval_seconds: u64,
    pub recent_scan: Option<String>,
    pub recent_continuation: Option<String>,
    pub recent_errors: Vec<String>,
    pub log_directory: String,
    pub data_directory: String,
    pub backup_count: usize,
    pub generated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseHealth {
    pub path: String,
    pub schema_version: i64,
    pub integrity: String,
    pub size_bytes: u64,
    pub orphan_nodes: usize,
    pub invalid_bindings: usize,
    pub checked_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseBackupRecord {
    pub id: String,
    pub path: String,
    pub reason: String,
    pub schema_version: i64,
    pub size_bytes: u64,
    pub sha256: String,
    pub created_at: String,
    pub restored_at: Option<String>,
}
