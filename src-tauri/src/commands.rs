use crate::{
    codex_app_server, codex_runtime, configuration, context_compiler, continuation, database,
    diagnostics,
    error::{AppError, AppResult},
    filesystem,
    models::*,
    package_builder, package_validator, profiles, search, session_indexer, session_scanner,
    settings, unified_project, AppState,
};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tauri::State;

#[tauri::command]
pub fn get_dashboard(state: State<'_, AppState>) -> AppResult<DashboardStats> {
    database::dashboard(&state.db_path)
}

#[tauri::command(async)]
pub fn list_sessions(state: State<'_, AppState>) -> AppResult<Vec<SessionSummary>> {
    database::list_sessions(&state.db_path)
}

#[tauri::command(async)]
pub fn get_session(state: State<'_, AppState>, id: String) -> AppResult<SessionDetail> {
    database::get_session(&state.db_path, &id)
}

#[tauri::command(async)]
pub fn scan_sessions(state: State<'_, AppState>) -> AppResult<Vec<SessionSummary>> {
    let settings = settings::load(&state.db_path, &state.data_dir)?;
    session_scanner::scan(&state.db_path, &settings)
}

#[tauri::command]
pub fn detect_codex_capabilities(
    state: State<'_, AppState>,
    force: bool,
) -> AppResult<CodexCapabilityReport> {
    codex_runtime::detect(&state.db_path, &state.data_dir, force)
}

#[tauri::command]
pub fn probe_codex_app_server(state: State<'_, AppState>) -> AppResult<String> {
    let report = codex_runtime::detect(&state.db_path, &state.data_dir, true)?;
    if !report.supports_app_server {
        return Err(AppError::Message("当前 Codex CLI 不支持 app-server".into()));
    }
    let command = report
        .executable_path
        .ok_or_else(|| AppError::Message("Codex 能力报告缺少可执行文件路径".into()))?;
    codex_app_server::probe(&command, state.data_dir.to_string_lossy().as_ref())
}

#[tauri::command(async)]
pub fn poll_session_changes(state: State<'_, AppState>) -> AppResult<WatchPollResult> {
    let settings = settings::load(&state.db_path, &state.data_dir)?;
    if !settings.auto_watch {
        return Ok(WatchPollResult::default());
    }
    session_indexer::poll(&state.db_path, &settings)
}

#[tauri::command(async)]
pub fn reindex_session(state: State<'_, AppState>, session_id: String) -> AppResult<SessionDetail> {
    let detail = database::get_session(&state.db_path, &session_id)?;
    let settings = settings::load(&state.db_path, &state.data_dir)?;
    session_indexer::full_index_file(
        &state.db_path,
        Path::new(&detail.summary.source_path),
        settings.read_git_state,
    )
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> AppResult<AppSettings> {
    settings::load(&state.db_path, &state.data_dir)
}

#[tauri::command]
pub fn save_settings(
    state: State<'_, AppState>,
    mut settings: AppSettings,
) -> AppResult<AppSettings> {
    settings.database_path = state.db_path.to_string_lossy().into_owned();
    settings::save(&state.db_path, &settings)
}

#[tauri::command]
pub fn prepare_package_draft(
    state: State<'_, AppState>,
    session_id: String,
) -> AppResult<PackageDraft> {
    Ok(package_builder::prepare_draft(&database::get_session(
        &state.db_path,
        &session_id,
    )?))
}

#[tauri::command]
pub fn create_package(
    state: State<'_, AppState>,
    draft: PackageDraft,
) -> AppResult<PackageSummary> {
    let settings = settings::load(&state.db_path, &state.data_dir)?;
    package_builder::build(&state.db_path, &settings, &draft)
}

#[tauri::command]
pub fn list_packages(state: State<'_, AppState>) -> AppResult<Vec<PackageSummary>> {
    database::list_packages(&state.db_path)
}

#[tauri::command]
pub fn get_package(state: State<'_, AppState>, id: String) -> AppResult<PackageDetail> {
    package_builder::load_detail(database::get_package_summary(&state.db_path, &id)?)
}

#[tauri::command]
pub fn validate_package(state: State<'_, AppState>, id: String) -> AppResult<ValidationReport> {
    let item = database::get_package_summary(&state.db_path, &id)?;
    package_validator::validate(Path::new(&item.package_path))
}

#[tauri::command]
pub fn import_package(state: State<'_, AppState>, path: String) -> AppResult<PackageSummary> {
    let settings = settings::load(&state.db_path, &state.data_dir)?;
    package_builder::import(&state.db_path, &settings, Path::new(&path))
}

#[tauri::command]
pub fn export_package_zip(
    state: State<'_, AppState>,
    id: String,
    destination: Option<String>,
) -> AppResult<String> {
    let item = database::get_package_summary(&state.db_path, &id)?;
    let default = PathBuf::from(&item.package_path).with_extension("agentpack.zip");
    let target = destination.map(PathBuf::from).unwrap_or(default);
    if !target.exists() {
        filesystem::create_zip(Path::new(&item.package_path), &target)?;
    }
    Ok(target.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn export_package_folder(
    state: State<'_, AppState>,
    id: String,
    destination: String,
) -> AppResult<String> {
    let item = database::get_package_summary(&state.db_path, &id)?;
    let destination = PathBuf::from(destination).join(format!(
        "{}-agentpack",
        item.title
            .chars()
            .map(
                |ch| if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                    ch
                } else {
                    '-'
                }
            )
            .collect::<String>()
    ));
    if destination.exists() {
        return Err(AppError::Message("导出目标文件夹已存在".into()));
    }
    filesystem::copy_directory(Path::new(&item.package_path), &destination)?;
    Ok(destination.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn delete_package(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let item = database::get_package_summary(&state.db_path, &id)?;
    let settings = settings::load(&state.db_path, &state.data_dir)?;
    let package_path = PathBuf::from(&item.package_path);
    let root = database::canonical_package_root(&settings);
    if !filesystem::is_within(&package_path, &root) {
        return Err(AppError::Message("拒绝删除任务包根目录以外的路径".into()));
    }
    if package_path.is_dir() {
        fs::remove_dir_all(&package_path)?;
    }
    let zip = package_path.with_extension("agentpack.zip");
    if zip.is_file() && filesystem::is_within(&zip, &root) {
        fs::remove_file(zip)?;
    }
    database::delete_package_record(&state.db_path, &id)
}

#[tauri::command]
pub fn mark_package_resumed(state: State<'_, AppState>, id: String) -> AppResult<()> {
    database::mark_resumed(&state.db_path, &id)
}

#[tauri::command]
pub fn list_projects(state: State<'_, AppState>) -> AppResult<Vec<UnifiedProjectSummary>> {
    let app_settings = settings::load(&state.db_path, &state.data_dir)?;
    unified_project::list(&state.db_path, app_settings.default_context_budget)
}
#[tauri::command]
pub fn create_project(
    state: State<'_, AppState>,
    input: CreateProjectInput,
) -> AppResult<UnifiedProjectDetail> {
    let app_settings = settings::load(&state.db_path, &state.data_dir)?;
    unified_project::create(&state.db_path, &input, app_settings.default_context_budget)
}
#[tauri::command]
pub fn get_project(state: State<'_, AppState>, id: String) -> AppResult<UnifiedProjectDetail> {
    let app_settings = settings::load(&state.db_path, &state.data_dir)?;
    unified_project::get(&state.db_path, &id, app_settings.default_context_budget)
}
#[tauri::command]
pub fn archive_project(state: State<'_, AppState>, id: String) -> AppResult<()> {
    unified_project::archive(&state.db_path, &id)
}

#[tauri::command]
pub fn restore_project(state: State<'_, AppState>, id: String) -> AppResult<()> {
    unified_project::restore_project(&state.db_path, &id)
}

#[tauri::command]
pub fn rename_project(state: State<'_, AppState>, id: String, name: String) -> AppResult<()> {
    unified_project::rename_project(&state.db_path, &id, &name)
}

#[tauri::command]
pub fn relocate_project(
    state: State<'_, AppState>,
    id: String,
    project_path: String,
) -> AppResult<()> {
    unified_project::relocate_project(&state.db_path, &id, &project_path)
}

#[tauri::command]
pub fn delete_project_record(state: State<'_, AppState>, id: String) -> AppResult<()> {
    unified_project::delete_project_record(&state.db_path, &id)
}

#[tauri::command]
pub fn unbind_project_session(
    state: State<'_, AppState>,
    project_id: String,
    session_id: String,
) -> AppResult<()> {
    unified_project::unbind_session(&state.db_path, &project_id, &session_id)
}

#[tauri::command]
pub fn rebind_project_session(
    state: State<'_, AppState>,
    session_id: String,
    project_id: String,
    branch_id: String,
) -> AppResult<UnifiedProjectDetail> {
    let settings = settings::load(&state.db_path, &state.data_dir)?;
    unified_project::rebind_session(
        &state.db_path,
        &session_id,
        &project_id,
        &branch_id,
        settings.default_context_budget,
    )
}

#[tauri::command]
pub fn suggest_projects_for_session(
    state: State<'_, AppState>,
    session_id: String,
) -> AppResult<Vec<UnifiedProjectSummary>> {
    let settings = settings::load(&state.db_path, &state.data_dir)?;
    unified_project::suggested_projects(
        &state.db_path,
        &session_id,
        settings.default_context_budget,
    )
}

#[tauri::command]
pub fn check_database(state: State<'_, AppState>) -> AppResult<DatabaseHealth> {
    database::health(&state.db_path)
}

#[tauri::command]
pub fn create_database_backup(
    state: State<'_, AppState>,
    reason: Option<String>,
) -> AppResult<DatabaseBackupRecord> {
    let settings = settings::load(&state.db_path, &state.data_dir)?;
    let directory = if settings.backup_directory.trim().is_empty() {
        state.data_dir.join("backups")
    } else {
        PathBuf::from(settings.backup_directory)
    };
    database::backup(
        &state.db_path,
        &directory,
        reason.as_deref().unwrap_or("manual"),
    )
}

#[tauri::command]
pub fn list_database_backups(state: State<'_, AppState>) -> AppResult<Vec<DatabaseBackupRecord>> {
    database::list_backups(&state.db_path)
}

#[tauri::command]
pub fn restore_database_backup(
    state: State<'_, AppState>,
    backup_path: String,
) -> AppResult<DatabaseHealth> {
    let settings = settings::load(&state.db_path, &state.data_dir)?;
    let directory = if settings.backup_directory.trim().is_empty() {
        state.data_dir.join("backups")
    } else {
        PathBuf::from(settings.backup_directory)
    };
    database::restore(&state.db_path, Path::new(&backup_path), &directory)
}
#[tauri::command]
pub fn get_diagnostics(
    state: State<'_, AppState>,
    force_codex: bool,
) -> AppResult<DiagnosticsReport> {
    diagnostics::collect(&state.db_path, &state.data_dir, force_codex)
}
#[tauri::command]
pub fn copy_diagnostics_report(state: State<'_, AppState>) -> AppResult<String> {
    let report = diagnostics::collect(&state.db_path, &state.data_dir, false)?;
    diagnostics::sanitized_json(&report)
}
#[tauri::command]
pub fn export_diagnostics_report(state: State<'_, AppState>, path: String) -> AppResult<String> {
    let report = diagnostics::collect(&state.db_path, &state.data_dir, false)?;
    diagnostics::export(&report, Path::new(&path))
}
#[tauri::command]
pub fn validate_settings_paths(
    state: State<'_, AppState>,
    settings: AppSettings,
) -> AppResult<Vec<DiagnosticPathStatus>> {
    let _ = state;
    Ok(diagnostics::validate_settings_paths(&settings))
}
#[tauri::command]
pub fn bind_sessions_to_project(
    state: State<'_, AppState>,
    project_id: String,
    session_ids: Vec<String>,
    branch_id: Option<String>,
) -> AppResult<UnifiedProjectDetail> {
    let app_settings = settings::load(&state.db_path, &state.data_dir)?;
    unified_project::bind_sessions(
        &state.db_path,
        &project_id,
        &session_ids,
        branch_id.as_deref(),
        app_settings.default_context_budget,
    )
}
#[tauri::command]
pub fn get_unified_timeline(
    state: State<'_, AppState>,
    project_id: String,
    branch_id: String,
) -> AppResult<Vec<ConversationNode>> {
    unified_project::timeline(&state.db_path, &project_id, &branch_id)
}
#[tauri::command]
pub fn add_user_note(
    state: State<'_, AppState>,
    project_id: String,
    branch_id: String,
    content: String,
    parent_node_id: Option<String>,
) -> AppResult<ConversationNode> {
    unified_project::add_note(
        &state.db_path,
        &project_id,
        &branch_id,
        &content,
        parent_node_id.as_deref(),
    )
}
#[tauri::command]
pub fn create_conversation_branch(
    state: State<'_, AppState>,
    project_id: String,
    from_node_id: String,
    name: String,
) -> AppResult<ConversationBranch> {
    unified_project::create_branch(&state.db_path, &project_id, &from_node_id, &name)
}
#[tauri::command]
pub fn update_conversation_node(
    state: State<'_, AppState>,
    node_id: String,
    status: String,
    importance: i32,
) -> AppResult<ConversationNode> {
    unified_project::update_node(&state.db_path, &node_id, &status, importance)
}
#[tauri::command]
pub fn rename_conversation_branch(
    state: State<'_, AppState>,
    branch_id: String,
    name: String,
) -> AppResult<()> {
    unified_project::rename_branch(&state.db_path, &branch_id, &name)
}
#[tauri::command]
pub fn archive_conversation_branch(state: State<'_, AppState>, branch_id: String) -> AppResult<()> {
    unified_project::archive_branch(&state.db_path, &branch_id)
}
#[tauri::command]
pub fn restore_conversation_branch(state: State<'_, AppState>, branch_id: String) -> AppResult<()> {
    unified_project::restore_branch(&state.db_path, &branch_id)
}
#[tauri::command]
pub fn switch_conversation_branch(
    state: State<'_, AppState>,
    project_id: String,
    branch_id: String,
) -> AppResult<()> {
    unified_project::switch_branch(&state.db_path, &project_id, &branch_id)
}
#[tauri::command]
pub fn delete_conversation_branch(state: State<'_, AppState>, branch_id: String) -> AppResult<()> {
    unified_project::delete_branch(&state.db_path, &branch_id)
}
#[tauri::command]
pub fn compare_conversation_branches(
    state: State<'_, AppState>,
    source_branch_id: String,
    target_branch_id: String,
) -> AppResult<BranchComparison> {
    unified_project::compare_branches(&state.db_path, &source_branch_id, &target_branch_id)
}
#[tauri::command]
pub fn merge_branch_context_items(
    state: State<'_, AppState>,
    source_branch_id: String,
    target_branch_id: String,
    node_ids: Vec<String>,
) -> AppResult<ConversationNode> {
    unified_project::merge_branch_nodes(
        &state.db_path,
        &source_branch_id,
        &target_branch_id,
        &node_ids,
    )
}
#[tauri::command]
pub fn global_search(
    state: State<'_, AppState>,
    query: String,
    limit: Option<usize>,
) -> AppResult<Vec<GlobalSearchResult>> {
    search::global(&state.db_path, &query, limit.unwrap_or(80))
}
#[tauri::command]
pub fn sync_project_sessions(state: State<'_, AppState>, project_id: String) -> AppResult<usize> {
    unified_project::sync(&state.db_path, &project_id)
}
#[tauri::command]
pub fn compile_context(
    state: State<'_, AppState>,
    options: ContextCompileOptions,
) -> AppResult<CompiledContext> {
    context_compiler::compile(&state.db_path, &options)
}
#[tauri::command]
pub fn save_context_snapshot(
    state: State<'_, AppState>,
    options: ContextCompileOptions,
) -> AppResult<ContextSnapshot> {
    context_compiler::save_snapshot(&state.db_path, &options)
}
#[tauri::command]
pub fn list_context_snapshots(
    state: State<'_, AppState>,
    project_id: String,
) -> AppResult<Vec<ContextSnapshot>> {
    context_compiler::list_snapshots(&state.db_path, &project_id)
}
#[tauri::command]
pub fn diff_context_snapshots(
    state: State<'_, AppState>,
    from_snapshot_id: String,
    to_snapshot_id: String,
) -> AppResult<ContextSnapshotDiff> {
    context_compiler::diff_snapshots(&state.db_path, &from_snapshot_id, &to_snapshot_id)
}
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn set_context_item_override(
    state: State<'_, AppState>,
    project_id: String,
    branch_id: Option<String>,
    source_node_id: Option<String>,
    content_hash: String,
    action: Option<String>,
    priority: Option<i32>,
    pinned: Option<bool>,
    stale: Option<bool>,
    incorrect: Option<bool>,
    permanent: bool,
) -> AppResult<()> {
    context_compiler::set_item_override(
        &state.db_path,
        &project_id,
        branch_id.as_deref(),
        source_node_id.as_deref(),
        &content_hash,
        action.as_deref(),
        priority,
        pinned,
        stale,
        incorrect,
        permanent,
    )
}
#[tauri::command]
pub fn create_continuation(
    state: State<'_, AppState>,
    options: ContextCompileOptions,
    launch: bool,
) -> AppResult<ContinuationRecord> {
    continuation::create(
        &state.db_path,
        &state.data_dir,
        &options,
        launch,
        &state.app_server,
    )
}
#[tauri::command]
pub fn launch_continuation(
    state: State<'_, AppState>,
    continuation_id: String,
) -> AppResult<ContinuationRecord> {
    continuation::launch_prepared(
        &state.db_path,
        &state.data_dir,
        &continuation_id,
        &state.app_server,
    )
}
#[tauri::command]
pub fn list_continuations(
    state: State<'_, AppState>,
    project_id: String,
) -> AppResult<Vec<ContinuationRecord>> {
    continuation::list(&state.db_path, &project_id)
}
#[tauri::command]
pub fn poll_continuation(
    state: State<'_, AppState>,
    continuation_id: String,
) -> AppResult<ContinuationPollResult> {
    continuation::poll(&state.db_path, &state.data_dir, &continuation_id)
}
#[tauri::command]
pub fn bind_continuation_session(
    state: State<'_, AppState>,
    continuation_id: String,
    session_id: String,
) -> AppResult<()> {
    continuation::bind_manual(&state.db_path, &continuation_id, &session_id)
}
#[tauri::command]
pub fn cancel_continuation(
    state: State<'_, AppState>,
    continuation_id: String,
) -> AppResult<ContinuationRecord> {
    continuation::cancel(&state.db_path, &continuation_id)
}
#[tauri::command]
pub fn retry_continuation(
    state: State<'_, AppState>,
    continuation_id: String,
) -> AppResult<ContinuationRecord> {
    continuation::retry(
        &state.db_path,
        &state.data_dir,
        &continuation_id,
        &state.app_server,
    )
}

#[tauri::command]
pub fn list_app_server_requests(
    state: State<'_, AppState>,
) -> AppResult<Vec<AppServerClientRequest>> {
    state.app_server.list_requests()
}

#[tauri::command]
pub fn respond_app_server_request(
    state: State<'_, AppState>,
    request_id: String,
    response: serde_json::Value,
) -> AppResult<()> {
    state.app_server.respond(&request_id, response)
}
#[tauri::command]
pub fn recover_continuations(state: State<'_, AppState>) -> AppResult<Vec<ContinuationRecord>> {
    continuation::recover(&state.db_path)
}
#[tauri::command]
pub fn cleanup_continuation_context(
    state: State<'_, AppState>,
    continuation_id: String,
) -> AppResult<ContinuationRecord> {
    continuation::cleanup_context_file(&state.db_path, &continuation_id)
}
#[tauri::command]
pub fn launch_source_session(
    state: State<'_, AppState>,
    session_id: String,
    operation: String,
) -> AppResult<u32> {
    continuation::launch_source(&state.db_path, &state.data_dir, &session_id, &operation)
}
#[tauri::command]
pub fn scan_configurations(
    state: State<'_, AppState>,
    project_id: Option<String>,
) -> AppResult<ConfigurationInventory> {
    configuration::scan(&state.db_path, project_id.as_deref())
}
#[tauri::command]
pub fn bind_configuration(
    state: State<'_, AppState>,
    project_id: String,
    kind: String,
    item_id: String,
    bound: bool,
) -> AppResult<()> {
    configuration::set_binding(&state.db_path, &project_id, &kind, &item_id, bound)
}

#[tauri::command]
pub fn list_codex_profiles(
    state: State<'_, AppState>,
    project_id: Option<String>,
) -> AppResult<Vec<CodexProfile>> {
    profiles::list(&state.db_path, project_id.as_deref())
}

#[tauri::command]
pub fn create_default_codex_profile(
    state: State<'_, AppState>,
    project_id: Option<String>,
    branch_id: Option<String>,
) -> AppResult<CodexProfile> {
    let app_settings = settings::load(&state.db_path, &state.data_dir)?;
    let working_directory = if let Some(id) = project_id.as_deref() {
        unified_project::get(&state.db_path, id, app_settings.default_context_budget)?
            .summary
            .project_path
    } else if !app_settings.default_working_directory.trim().is_empty() {
        app_settings.default_working_directory.clone()
    } else {
        std::env::current_dir()?.to_string_lossy().into_owned()
    };
    let capabilities = codex_runtime::detect(&state.db_path, &state.data_dir, false)?;
    let profile = profiles::default_profile(
        project_id,
        branch_id,
        working_directory,
        &capabilities,
        app_settings.default_context_budget,
    );
    profiles::save(&state.db_path, profile, &capabilities)
}

#[tauri::command]
pub fn save_codex_profile(
    state: State<'_, AppState>,
    profile: CodexProfile,
) -> AppResult<CodexProfile> {
    let capabilities = codex_runtime::detect(&state.db_path, &state.data_dir, false)?;
    profiles::save(&state.db_path, profile, &capabilities)
}

#[tauri::command]
pub fn duplicate_codex_profile(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> AppResult<CodexProfile> {
    profiles::duplicate(&state.db_path, &id, &name)
}

#[tauri::command]
pub fn delete_codex_profile(state: State<'_, AppState>, id: String) -> AppResult<()> {
    profiles::delete(&state.db_path, &id)
}

#[tauri::command]
pub fn set_project_codex_profile(
    state: State<'_, AppState>,
    project_id: String,
    profile_id: String,
) -> AppResult<()> {
    profiles::set_project_default(&state.db_path, &project_id, &profile_id)
}

#[tauri::command]
pub fn set_branch_codex_profile(
    state: State<'_, AppState>,
    project_id: String,
    branch_id: String,
    profile_id: String,
) -> AppResult<()> {
    profiles::set_branch_default(&state.db_path, &project_id, &branch_id, &profile_id)
}

#[tauri::command]
pub fn export_codex_profile(
    state: State<'_, AppState>,
    id: String,
    path: String,
) -> AppResult<String> {
    profiles::export_profile(&state.db_path, &id, Path::new(&path))
}

#[tauri::command]
pub fn import_codex_profile(state: State<'_, AppState>, path: String) -> AppResult<CodexProfile> {
    let capabilities = codex_runtime::detect(&state.db_path, &state.data_dir, false)?;
    profiles::import_profile(&state.db_path, Path::new(&path), &capabilities)
}
