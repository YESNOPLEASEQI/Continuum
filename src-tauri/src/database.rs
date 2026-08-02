use crate::{
    error::{AppError, AppResult},
    filesystem,
    models::*,
};
use rusqlite::{params, Connection, OptionalExtension};
use std::fs;
use std::path::{Path, PathBuf};

pub const LATEST_SCHEMA_VERSION: i64 = 4;

pub const MIGRATION_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS schema_migrations (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS app_settings (key TEXT PRIMARY KEY, value_json TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS agent_installations (id TEXT PRIMARY KEY, agent_type TEXT NOT NULL, path TEXT NOT NULL, detected_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sessions (id TEXT PRIMARY KEY, agent_type TEXT NOT NULL, title TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, working_directory TEXT, git_repository TEXT, source_path TEXT NOT NULL, detail_json TEXT NOT NULL, parse_warning TEXT);
CREATE TABLE IF NOT EXISTS session_messages (id TEXT PRIMARY KEY, session_id TEXT NOT NULL, role TEXT NOT NULL, content TEXT NOT NULL, timestamp TEXT, FOREIGN KEY(session_id) REFERENCES sessions(id) ON DELETE CASCADE);
CREATE TABLE IF NOT EXISTS session_tool_calls (id TEXT PRIMARY KEY, session_id TEXT NOT NULL, name TEXT NOT NULL, arguments TEXT NOT NULL, status TEXT NOT NULL, output TEXT, timestamp TEXT, FOREIGN KEY(session_id) REFERENCES sessions(id) ON DELETE CASCADE);
CREATE TABLE IF NOT EXISTS packages (id TEXT PRIMARY KEY, title TEXT NOT NULL, source_agent TEXT NOT NULL, target_agent TEXT NOT NULL, created_at TEXT NOT NULL, project_path TEXT, package_path TEXT NOT NULL, schema_version TEXT NOT NULL, integrity TEXT NOT NULL, has_patch INTEGER NOT NULL, security_warning_count INTEGER NOT NULL, imported INTEGER NOT NULL, resumed INTEGER NOT NULL DEFAULT 0);
CREATE TABLE IF NOT EXISTS package_files (package_id TEXT NOT NULL, relative_path TEXT NOT NULL, sha256 TEXT NOT NULL, size INTEGER NOT NULL, PRIMARY KEY(package_id, relative_path), FOREIGN KEY(package_id) REFERENCES packages(id) ON DELETE CASCADE);
CREATE TABLE IF NOT EXISTS scan_jobs (id TEXT PRIMARY KEY, started_at TEXT NOT NULL, completed_at TEXT, status TEXT NOT NULL, discovered_count INTEGER NOT NULL DEFAULT 0, error TEXT);
CREATE TABLE IF NOT EXISTS security_findings (id INTEGER PRIMARY KEY AUTOINCREMENT, package_id TEXT NOT NULL, finding_type TEXT NOT NULL, source_file TEXT NOT NULL, field_path TEXT NOT NULL, severity TEXT NOT NULL, FOREIGN KEY(package_id) REFERENCES packages(id) ON DELETE CASCADE);
CREATE INDEX IF NOT EXISTS idx_sessions_updated ON sessions(updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_session_messages_session ON session_messages(session_id);
CREATE INDEX IF NOT EXISTS idx_session_tool_calls_session ON session_tool_calls(session_id);
CREATE INDEX IF NOT EXISTS idx_packages_created ON packages(created_at DESC);
CREATE TABLE IF NOT EXISTS projects (
  id TEXT PRIMARY KEY, name TEXT NOT NULL, project_path TEXT NOT NULL, git_repository TEXT,
  goal TEXT NOT NULL, constraints_json TEXT NOT NULL, default_agent TEXT NOT NULL,
  default_model TEXT NOT NULL, current_branch_id TEXT NOT NULL, current_task TEXT NOT NULL DEFAULT '',
  archived INTEGER NOT NULL DEFAULT 0, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS conversation_branches (
  id TEXT PRIMARY KEY, project_id TEXT NOT NULL, name TEXT NOT NULL, parent_branch_id TEXT,
  fork_node_id TEXT, status TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
  FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS conversation_nodes (
  id TEXT PRIMARY KEY, project_id TEXT NOT NULL, parent_node_id TEXT, branch_id TEXT NOT NULL,
  source_agent TEXT, source_session_id TEXT, node_type TEXT NOT NULL, content TEXT NOT NULL,
  created_at TEXT NOT NULL, importance INTEGER NOT NULL DEFAULT 50, status TEXT NOT NULL DEFAULT 'active',
  metadata_json TEXT NOT NULL DEFAULT '{}',
  FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE,
  FOREIGN KEY(branch_id) REFERENCES conversation_branches(id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS source_sessions (
  id TEXT PRIMARY KEY, agent_type TEXT NOT NULL, title TEXT NOT NULL, source_path TEXT NOT NULL,
  working_directory TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, detail_json TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS source_messages (
  id TEXT PRIMARY KEY, source_session_id TEXT NOT NULL, role TEXT NOT NULL, content TEXT NOT NULL,
  created_at TEXT, raw_index INTEGER NOT NULL, FOREIGN KEY(source_session_id) REFERENCES source_sessions(id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS tool_calls_v2 (
  id TEXT PRIMARY KEY, source_session_id TEXT NOT NULL, name TEXT NOT NULL, arguments TEXT NOT NULL,
  status TEXT NOT NULL, output TEXT, created_at TEXT, FOREIGN KEY(source_session_id) REFERENCES source_sessions(id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS file_changes (
  id TEXT PRIMARY KEY, source_session_id TEXT NOT NULL, path TEXT NOT NULL, change_type TEXT NOT NULL,
  created_at TEXT NOT NULL, FOREIGN KEY(source_session_id) REFERENCES source_sessions(id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS project_bindings (
  project_id TEXT NOT NULL, binding_type TEXT NOT NULL, binding_id TEXT NOT NULL, branch_id TEXT,
  created_at TEXT NOT NULL, metadata_json TEXT NOT NULL DEFAULT '{}',
  PRIMARY KEY(project_id,binding_type,binding_id), FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS context_snapshots (
  id TEXT PRIMARY KEY, project_id TEXT NOT NULL, branch_id TEXT NOT NULL, source_node_id TEXT,
  target_agent TEXT NOT NULL, target_model TEXT NOT NULL, token_budget INTEGER NOT NULL,
  estimated_tokens INTEGER NOT NULL, compiled_context TEXT NOT NULL, compiled_json TEXT NOT NULL,
  created_at TEXT NOT NULL, FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS context_items (
  id TEXT PRIMARY KEY, snapshot_id TEXT NOT NULL, source_node_id TEXT, category TEXT NOT NULL,
  action TEXT NOT NULL, reason TEXT NOT NULL, estimated_tokens INTEGER NOT NULL,
  content TEXT NOT NULL, pinned INTEGER NOT NULL DEFAULT 0,
  FOREIGN KEY(snapshot_id) REFERENCES context_snapshots(id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS continuations (
  id TEXT PRIMARY KEY, project_id TEXT NOT NULL, branch_id TEXT NOT NULL, source_node_id TEXT,
  snapshot_id TEXT NOT NULL, target_agent TEXT NOT NULL, target_model TEXT NOT NULL,
  mode TEXT NOT NULL, status TEXT NOT NULL, bootstrap_file TEXT NOT NULL, launch_command TEXT NOT NULL,
  target_session_id TEXT, created_at TEXT NOT NULL, warning TEXT, process_id INTEGER,
  working_directory TEXT NOT NULL DEFAULT '', context_hash TEXT NOT NULL DEFAULT '',
  marker TEXT NOT NULL DEFAULT '', started_at TEXT NOT NULL DEFAULT '', detected_at TEXT,
  listening INTEGER NOT NULL DEFAULT 0,
  FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS agent_capabilities (
  agent_type TEXT PRIMARY KEY, capabilities_json TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS skills (
  id TEXT PRIMARY KEY, name TEXT NOT NULL, description TEXT NOT NULL, source_platform TEXT NOT NULL,
  source_path TEXT NOT NULL, compatible_agents_json TEXT NOT NULL, required_tools_json TEXT NOT NULL,
  instructions TEXT NOT NULL, installation_state TEXT NOT NULL, discovered_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS mcp_servers (
  id TEXT PRIMARY KEY, name TEXT NOT NULL, source_agent TEXT NOT NULL, command TEXT,
  transport TEXT NOT NULL, compatible_agents_json TEXT NOT NULL, discovered_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_nodes_project_branch ON conversation_nodes(project_id,branch_id,created_at);
CREATE INDEX IF NOT EXISTS idx_nodes_source_session ON conversation_nodes(source_session_id);
CREATE INDEX IF NOT EXISTS idx_file_changes_session ON file_changes(source_session_id);
CREATE INDEX IF NOT EXISTS idx_bindings_project ON project_bindings(project_id,binding_type);
CREATE INDEX IF NOT EXISTS idx_snapshots_project ON context_snapshots(project_id,created_at DESC);
"#;

const MIGRATION_V3_SQL: &str = r#"
CREATE UNIQUE INDEX IF NOT EXISTS idx_projects_normalized_path
  ON projects(normalized_path) WHERE normalized_path <> '';
CREATE UNIQUE INDEX IF NOT EXISTS idx_nodes_source_identity
  ON conversation_nodes(source_agent,source_session_id,source_message_id)
  WHERE source_agent IS NOT NULL AND source_session_id IS NOT NULL AND source_message_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_source_sessions_file ON source_sessions(session_file_path);
CREATE INDEX IF NOT EXISTS idx_source_sessions_cwd ON source_sessions(normalized_working_directory);
CREATE INDEX IF NOT EXISTS idx_source_sessions_binding ON source_sessions(bound_project_id,bound_branch_id);
CREATE INDEX IF NOT EXISTS idx_continuations_status ON continuations(status,updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_context_items_snapshot_action ON context_items(snapshot_id,action,priority DESC);

CREATE TABLE IF NOT EXISTS codex_profiles (
  id TEXT PRIMARY KEY, project_id TEXT, branch_id TEXT, name TEXT NOT NULL,
  executable_path TEXT NOT NULL, model TEXT, working_directory TEXT NOT NULL,
  approval_mode TEXT NOT NULL, sandbox_mode TEXT NOT NULL, launch_arguments_json TEXT NOT NULL,
  context_budget INTEGER NOT NULL, recent_message_limit INTEGER NOT NULL,
  include_git_status INTEGER NOT NULL, include_git_diff INTEGER NOT NULL,
  include_tests INTEGER NOT NULL, include_failed_attempts INTEGER NOT NULL,
  include_skills INTEGER NOT NULL, include_mcp INTEGER NOT NULL,
  launch_prompt_template TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
  FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE,
  FOREIGN KEY(branch_id) REFERENCES conversation_branches(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_codex_profiles_project ON codex_profiles(project_id,updated_at DESC);

CREATE TABLE IF NOT EXISTS project_skill_bindings (
  project_id TEXT NOT NULL, skill_id TEXT NOT NULL, created_at TEXT NOT NULL,
  PRIMARY KEY(project_id,skill_id),
  FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE,
  FOREIGN KEY(skill_id) REFERENCES skills(id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS branch_skill_bindings (
  branch_id TEXT NOT NULL, skill_id TEXT NOT NULL, created_at TEXT NOT NULL,
  PRIMARY KEY(branch_id,skill_id),
  FOREIGN KEY(branch_id) REFERENCES conversation_branches(id) ON DELETE CASCADE,
  FOREIGN KEY(skill_id) REFERENCES skills(id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS continuation_skill_bindings (
  continuation_id TEXT NOT NULL, skill_id TEXT NOT NULL, created_at TEXT NOT NULL,
  PRIMARY KEY(continuation_id,skill_id),
  FOREIGN KEY(continuation_id) REFERENCES continuations(id) ON DELETE CASCADE,
  FOREIGN KEY(skill_id) REFERENCES skills(id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS project_mcp_bindings (
  project_id TEXT NOT NULL, mcp_server_id TEXT NOT NULL, created_at TEXT NOT NULL,
  PRIMARY KEY(project_id,mcp_server_id),
  FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE,
  FOREIGN KEY(mcp_server_id) REFERENCES mcp_servers(id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS branch_mcp_bindings (
  branch_id TEXT NOT NULL, mcp_server_id TEXT NOT NULL, created_at TEXT NOT NULL,
  PRIMARY KEY(branch_id,mcp_server_id),
  FOREIGN KEY(branch_id) REFERENCES conversation_branches(id) ON DELETE CASCADE,
  FOREIGN KEY(mcp_server_id) REFERENCES mcp_servers(id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS continuation_mcp_bindings (
  continuation_id TEXT NOT NULL, mcp_server_id TEXT NOT NULL, created_at TEXT NOT NULL,
  PRIMARY KEY(continuation_id,mcp_server_id),
  FOREIGN KEY(continuation_id) REFERENCES continuations(id) ON DELETE CASCADE,
  FOREIGN KEY(mcp_server_id) REFERENCES mcp_servers(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS continuation_candidates (
  continuation_id TEXT NOT NULL, session_id TEXT NOT NULL, session_file_path TEXT NOT NULL,
  normalized_working_directory TEXT NOT NULL, created_at TEXT NOT NULL, modified_at TEXT NOT NULL,
  first_user_message TEXT NOT NULL, confidence INTEGER NOT NULL, validation_json TEXT NOT NULL,
  discovered_at TEXT NOT NULL, selected INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY(continuation_id,session_id),
  FOREIGN KEY(continuation_id) REFERENCES continuations(id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS session_scan_errors (
  id INTEGER PRIMARY KEY AUTOINCREMENT, session_file_path TEXT NOT NULL,
  line_number INTEGER, byte_offset INTEGER, error_code TEXT NOT NULL, message TEXT NOT NULL,
  occurred_at TEXT NOT NULL, resolved_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_session_scan_errors_file ON session_scan_errors(session_file_path,occurred_at DESC);

CREATE TABLE IF NOT EXISTS activity_events (
  id TEXT PRIMARY KEY, project_id TEXT, branch_id TEXT, event_type TEXT NOT NULL,
  entity_id TEXT, summary TEXT NOT NULL, metadata_json TEXT NOT NULL DEFAULT '{}', created_at TEXT NOT NULL,
  FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE,
  FOREIGN KEY(branch_id) REFERENCES conversation_branches(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_activity_project ON activity_events(project_id,created_at DESC);

CREATE TABLE IF NOT EXISTS continuation_templates (
  id TEXT PRIMARY KEY, name TEXT NOT NULL UNIQUE, description TEXT NOT NULL,
  options_json TEXT NOT NULL, builtin INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS context_item_overrides (
  id TEXT PRIMARY KEY, project_id TEXT NOT NULL, branch_id TEXT,
  source_node_id TEXT, content_hash TEXT NOT NULL, action TEXT,
  priority INTEGER, pinned INTEGER, stale INTEGER, incorrect INTEGER,
  permanent INTEGER NOT NULL DEFAULT 0, created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
  FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE,
  FOREIGN KEY(branch_id) REFERENCES conversation_branches(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_context_overrides_scope ON context_item_overrides(project_id,branch_id,content_hash);

CREATE TABLE IF NOT EXISTS database_backups (
  id TEXT PRIMARY KEY, path TEXT NOT NULL, reason TEXT NOT NULL, schema_version INTEGER NOT NULL,
  size_bytes INTEGER NOT NULL, sha256 TEXT NOT NULL, created_at TEXT NOT NULL,
  restored_at TEXT, validation_json TEXT NOT NULL DEFAULT '{}'
);
CREATE TABLE IF NOT EXISTS diagnostics_events (
  id TEXT PRIMARY KEY, level TEXT NOT NULL, area TEXT NOT NULL, code TEXT NOT NULL,
  message TEXT NOT NULL, metadata_json TEXT NOT NULL DEFAULT '{}', created_at TEXT NOT NULL,
  resolved_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_diagnostics_recent ON diagnostics_events(created_at DESC);
"#;

const MIGRATION_V4_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS app_server_notifications (
  notification_hash TEXT PRIMARY KEY, process_id INTEGER NOT NULL,
  continuation_id TEXT NOT NULL, project_id TEXT NOT NULL,
  thread_id TEXT, turn_id TEXT, item_id TEXT, method TEXT NOT NULL,
  emitted_at_ms INTEGER, processed_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_app_server_notifications_thread
  ON app_server_notifications(thread_id,processed_at);

CREATE TABLE IF NOT EXISTS app_server_turns (
  thread_id TEXT NOT NULL, turn_id TEXT NOT NULL, status TEXT NOT NULL,
  started_at TEXT, completed_at TEXT, error_json TEXT,
  updated_at TEXT NOT NULL,
  PRIMARY KEY(thread_id,turn_id)
);
CREATE INDEX IF NOT EXISTS idx_app_server_turns_thread
  ON app_server_turns(thread_id,updated_at);

CREATE TABLE IF NOT EXISTS app_server_items (
  thread_id TEXT NOT NULL, turn_id TEXT NOT NULL, item_id TEXT NOT NULL,
  item_type TEXT NOT NULL, status TEXT NOT NULL,
  role TEXT, content TEXT NOT NULL DEFAULT '',
  tool_name TEXT, arguments TEXT, output TEXT,
  started_at TEXT, completed_at TEXT, last_event_ms INTEGER,
  jsonl_verified INTEGER NOT NULL DEFAULT 0,
  jsonl_source_id TEXT, updated_at TEXT NOT NULL,
  PRIMARY KEY(thread_id,item_id)
);
CREATE INDEX IF NOT EXISTS idx_app_server_items_reconcile
  ON app_server_items(thread_id,jsonl_verified,started_at,item_id);
"#;

fn column_exists(conn: &Connection, table: &str, column: &str) -> AppResult<bool> {
    let mut statement = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(columns.iter().any(|value| value == column))
}

fn ensure_column(conn: &Connection, table: &str, definition: &str) -> AppResult<()> {
    let column = definition
        .split_whitespace()
        .next()
        .ok_or_else(|| AppError::Message("迁移列定义为空".into()))?;
    if !column_exists(conn, table, column)? {
        conn.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {definition};"))?;
    }
    Ok(())
}

fn pre_migration_backup(
    conn: &Connection,
    path: &Path,
    target_version: i64,
) -> AppResult<Option<PathBuf>> {
    if !path.is_file() || fs::metadata(path)?.len() == 0 {
        return Ok(None);
    }
    conn.execute_batch("PRAGMA wal_checkpoint(FULL);")?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let database_size = fs::metadata(path)?.len();
    let required_space = database_size.saturating_add(128 * 1024 * 1024);
    let available_space = available_disk_space(parent)?;
    if available_space < required_space {
        return Err(AppError::Message(format!(
            "数据库迁移需要先创建可恢复备份，但磁盘空间不足：至少需要 {} MiB，当前约 {} MiB 可用",
            required_space.div_ceil(1024 * 1024),
            available_space / (1024 * 1024)
        )));
    }
    let backup_dir = parent.join("backups");
    fs::create_dir_all(&backup_dir)?;
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("continuum");
    let timestamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    let backup = backup_dir.join(format!("{stem}-pre-v{target_version}-{timestamp}.sqlite3"));
    if let Err(error) = fs::copy(path, &backup) {
        let _ = fs::remove_file(&backup);
        return Err(AppError::Io(error));
    }
    Ok(Some(backup))
}

#[cfg(windows)]
fn available_disk_space(path: &Path) -> AppResult<u64> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    let mut wide = path.as_os_str().encode_wide().collect::<Vec<_>>();
    wide.push(0);
    let mut available = 0_u64;
    let result = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut available,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if result == 0 {
        return Err(AppError::Io(std::io::Error::last_os_error()));
    }
    Ok(available)
}

#[cfg(not(windows))]
fn available_disk_space(_path: &Path) -> AppResult<u64> {
    Ok(u64::MAX)
}

fn apply_v3(conn: &Connection) -> AppResult<()> {
    let project_columns = [
        "normalized_path TEXT NOT NULL DEFAULT ''",
        "display_path TEXT NOT NULL DEFAULT ''",
        "default_branch_id TEXT NOT NULL DEFAULT ''",
        "default_codex_profile_id TEXT",
        "last_opened_at TEXT",
        "deleted_at TEXT",
    ];
    for definition in project_columns {
        ensure_column(conn, "projects", definition)?;
    }
    for definition in [
        "forked_from_node_id TEXT",
        "current_session_id TEXT",
        "archived_at TEXT",
    ] {
        ensure_column(conn, "conversation_branches", definition)?;
    }
    for definition in [
        "source_message_id TEXT",
        "pinned INTEGER NOT NULL DEFAULT 0",
        "stale INTEGER NOT NULL DEFAULT 0",
        "incorrect INTEGER NOT NULL DEFAULT 0",
        "excluded INTEGER NOT NULL DEFAULT 0",
        "imported_at TEXT",
    ] {
        ensure_column(conn, "conversation_nodes", definition)?;
    }
    for definition in [
        "external_session_id TEXT NOT NULL DEFAULT ''",
        "session_file_path TEXT NOT NULL DEFAULT ''",
        "normalized_working_directory TEXT NOT NULL DEFAULT ''",
        "last_imported_offset INTEGER NOT NULL DEFAULT 0",
        "last_imported_line INTEGER NOT NULL DEFAULT 0",
        "pending_fragment TEXT NOT NULL DEFAULT ''",
        "file_hash TEXT NOT NULL DEFAULT ''",
        "bound_project_id TEXT",
        "bound_branch_id TEXT",
        "status TEXT NOT NULL DEFAULT 'indexed'",
        "raw_metadata TEXT NOT NULL DEFAULT '{}'",
        "file_created_at TEXT",
        "file_modified_at TEXT",
    ] {
        ensure_column(conn, "source_sessions", definition)?;
    }
    for definition in [
        "estimated_original_tokens INTEGER NOT NULL DEFAULT 0",
        "estimated_compiled_tokens INTEGER NOT NULL DEFAULT 0",
        "compiler_version TEXT NOT NULL DEFAULT 'rule-v1'",
        "content_hash TEXT NOT NULL DEFAULT ''",
        "metadata_json TEXT NOT NULL DEFAULT '{}'",
    ] {
        ensure_column(conn, "context_snapshots", definition)?;
    }
    for definition in [
        "action_reason TEXT NOT NULL DEFAULT ''",
        "priority INTEGER NOT NULL DEFAULT 50",
        "stale INTEGER NOT NULL DEFAULT 0",
        "incorrect INTEGER NOT NULL DEFAULT 0",
        "permanent INTEGER NOT NULL DEFAULT 0",
        "content_hash TEXT NOT NULL DEFAULT ''",
        "conflict_group_id TEXT",
    ] {
        ensure_column(conn, "context_items", definition)?;
    }
    for definition in [
        "source_session_id TEXT",
        "context_snapshot_id TEXT",
        "launch_started_at TEXT",
        "launch_process_id INTEGER",
        "launch_command_preview TEXT NOT NULL DEFAULT ''",
        "continuation_marker TEXT NOT NULL DEFAULT ''",
        "context_file_path TEXT NOT NULL DEFAULT ''",
        "context_file_hash TEXT NOT NULL DEFAULT ''",
        "failure_code TEXT",
        "failure_message TEXT",
        "updated_at TEXT NOT NULL DEFAULT ''",
        "completed_at TEXT",
        "detection_deadline_at TEXT",
        "retry_count INTEGER NOT NULL DEFAULT 0",
        "state_version INTEGER NOT NULL DEFAULT 0",
        "cancellation_requested INTEGER NOT NULL DEFAULT 0",
        "codex_profile_id TEXT",
        "launch_profile_json TEXT",
        "launch_transport TEXT NOT NULL DEFAULT 'cli'",
        "app_server_thread_id TEXT",
        "app_server_protocol_version TEXT",
    ] {
        ensure_column(conn, "continuations", definition)?;
    }
    for definition in [
        "description TEXT NOT NULL DEFAULT ''",
        "enabled INTEGER NOT NULL DEFAULT 1",
        "valid INTEGER NOT NULL DEFAULT 1",
        "duplicate_of TEXT",
        "dependencies_json TEXT NOT NULL DEFAULT '[]'",
        "modified_at TEXT",
    ] {
        ensure_column(conn, "skills", definition)?;
    }
    for definition in [
        "arguments_json TEXT NOT NULL DEFAULT '[]'",
        "environment_names_json TEXT NOT NULL DEFAULT '[]'",
        "enabled INTEGER NOT NULL DEFAULT 1",
        "valid INTEGER NOT NULL DEFAULT 1",
        "source_path TEXT NOT NULL DEFAULT ''",
        "duplicate_of TEXT",
        "modified_at TEXT",
    ] {
        ensure_column(conn, "mcp_servers", definition)?;
    }

    conn.execute("UPDATE projects SET display_path=CASE WHEN display_path='' THEN project_path ELSE display_path END, default_branch_id=CASE WHEN default_branch_id='' THEN current_branch_id ELSE default_branch_id END, last_opened_at=COALESCE(last_opened_at,updated_at)", [])?;
    let projects = {
        let mut statement =
            conn.prepare("SELECT id,project_path FROM projects ORDER BY created_at,id")?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };
    let mut claimed_paths = std::collections::BTreeSet::new();
    for (id, project_path) in projects {
        let key = filesystem::normalize_path_key(Path::new(&project_path));
        let normalized = if claimed_paths.insert(key.clone()) {
            key
        } else {
            conn.execute("UPDATE projects SET archived=1 WHERE id=?1", params![id])?;
            format!("{key}#legacy-duplicate-{id}")
        };
        conn.execute(
            "UPDATE projects SET normalized_path=?1 WHERE id=?2",
            params![normalized, id],
        )?;
    }
    conn.execute("UPDATE conversation_branches SET forked_from_node_id=COALESCE(forked_from_node_id,fork_node_id)", [])?;
    conn.execute("UPDATE conversation_nodes SET source_message_id=COALESCE(source_message_id,json_extract(metadata_json,'$.sourceMessageId')), imported_at=COALESCE(imported_at,created_at), pinned=CASE WHEN importance>=100 THEN 1 ELSE pinned END, stale=CASE WHEN status='stale' THEN 1 ELSE stale END, incorrect=CASE WHEN status='incorrect' THEN 1 ELSE incorrect END, excluded=CASE WHEN status='excluded' THEN 1 ELSE excluded END", [])?;
    conn.execute("UPDATE source_sessions SET external_session_id=CASE WHEN external_session_id='' THEN id ELSE external_session_id END, session_file_path=CASE WHEN session_file_path='' THEN source_path ELSE session_file_path END, bound_project_id=(SELECT project_id FROM project_bindings WHERE binding_type='source_session' AND binding_id=source_sessions.id LIMIT 1), bound_branch_id=(SELECT branch_id FROM project_bindings WHERE binding_type='source_session' AND binding_id=source_sessions.id LIMIT 1)", [])?;
    conn.execute("UPDATE context_snapshots SET estimated_compiled_tokens=CASE WHEN estimated_compiled_tokens=0 THEN estimated_tokens ELSE estimated_compiled_tokens END", [])?;
    conn.execute("UPDATE context_items SET action_reason=CASE WHEN action_reason='' THEN reason ELSE action_reason END", [])?;
    conn.execute("UPDATE continuations SET context_snapshot_id=COALESCE(context_snapshot_id,snapshot_id), launch_started_at=COALESCE(launch_started_at,NULLIF(started_at,'')), launch_process_id=COALESCE(launch_process_id,process_id), launch_command_preview=CASE WHEN launch_command_preview='' THEN launch_command ELSE launch_command_preview END, continuation_marker=CASE WHEN continuation_marker='' THEN marker ELSE continuation_marker END, context_file_path=CASE WHEN context_file_path='' THEN bootstrap_file ELSE context_file_path END, context_file_hash=CASE WHEN context_file_hash='' THEN context_hash ELSE context_file_hash END, updated_at=CASE WHEN updated_at='' THEN created_at ELSE updated_at END", [])?;
    conn.execute_batch(MIGRATION_V3_SQL)?;
    conn.execute(
        "INSERT OR IGNORE INTO schema_migrations(version,applied_at) VALUES(?1,?2)",
        params![3, chrono::Utc::now().to_rfc3339()],
    )?;
    conn.execute_batch("PRAGMA user_version=3;")?;
    Ok(())
}

fn apply_v4(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(MIGRATION_V4_SQL)?;
    conn.execute(
        "INSERT OR IGNORE INTO schema_migrations(version,applied_at) VALUES(4,?1)",
        params![chrono::Utc::now().to_rfc3339()],
    )?;
    conn.execute_batch("PRAGMA user_version=4;")?;
    Ok(())
}

pub fn initialize(path: &Path) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let existed = path.is_file()
        && fs::metadata(path)
            .map(|value| value.len() > 0)
            .unwrap_or(false);
    let conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    let transaction = conn.unchecked_transaction()?;
    transaction.execute_batch(MIGRATION_SQL)?;
    transaction.execute(
        "INSERT OR IGNORE INTO schema_migrations(version, applied_at) VALUES(1, ?1)",
        params![chrono::Utc::now().to_rfc3339()],
    )?;
    transaction.execute(
        "INSERT OR IGNORE INTO schema_migrations(version, applied_at) VALUES(2, ?1)",
        params![chrono::Utc::now().to_rfc3339()],
    )?;
    transaction.commit()?;
    let current_version: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version),0) FROM schema_migrations",
        [],
        |row| row.get(0),
    )?;
    if current_version < LATEST_SCHEMA_VERSION {
        let migration_backup = if existed {
            pre_migration_backup(&conn, path, LATEST_SCHEMA_VERSION)?
        } else {
            None
        };
        let transaction = conn.unchecked_transaction()?;
        if current_version < 3 {
            apply_v3(&transaction)?;
        }
        if current_version < 4 {
            apply_v4(&transaction)?;
        }
        transaction.commit()?;
        if let Some(backup) = migration_backup {
            let size = fs::metadata(&backup)?.len();
            conn.execute(
                "INSERT INTO database_backups(id,path,reason,schema_version,size_bytes,sha256,created_at) VALUES(?1,?2,'pre_migration',?3,?4,?5,?6)",
                params![
                    uuid::Uuid::new_v4().to_string(),
                    backup.to_string_lossy(),
                    current_version,
                    size as i64,
                    filesystem::sha256_file(&backup)?,
                    chrono::Utc::now().to_rfc3339()
                ],
            )?;
        }
    }
    Ok(())
}

pub fn connect(path: &Path) -> AppResult<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA busy_timeout=5000;")?;
    Ok(conn)
}

pub fn default_settings(db_path: &Path, data_dir: &Path) -> AppSettings {
    let default_session = dirs::home_dir().map(|home| home.join(".codex").join("sessions"));
    AppSettings {
        session_paths: default_session
            .filter(|path| path.exists())
            .map(|path| vec![path.to_string_lossy().into_owned()])
            .unwrap_or_default(),
        package_output_path: data_dir.join("packages").to_string_lossy().into_owned(),
        auto_scan: false,
        read_git_state: true,
        collect_command_logs: true,
        include_untracked_files: false,
        security_scan: true,
        theme: "dark".into(),
        database_path: db_path.to_string_lossy().into_owned(),
        log_level: "info".into(),
        default_working_directory: String::new(),
        ..AppSettings::default()
    }
}

pub fn get_settings(db_path: &Path, data_dir: &Path) -> AppResult<AppSettings> {
    let conn = connect(db_path)?;
    let value: Option<String> = conn
        .query_row(
            "SELECT value_json FROM app_settings WHERE key='settings'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    match value {
        Some(json) => Ok(serde_json::from_str(&json)?),
        None => {
            let settings = default_settings(db_path, data_dir);
            save_settings(db_path, &settings)?;
            Ok(settings)
        }
    }
}

pub fn save_settings(db_path: &Path, settings: &AppSettings) -> AppResult<()> {
    let conn = connect(db_path)?;
    conn.execute("INSERT INTO app_settings(key,value_json,updated_at) VALUES('settings',?1,?2) ON CONFLICT(key) DO UPDATE SET value_json=excluded.value_json,updated_at=excluded.updated_at", params![serde_json::to_string(settings)?, chrono::Utc::now().to_rfc3339()])?;
    Ok(())
}

pub fn upsert_session(db_path: &Path, detail: &SessionDetail) -> AppResult<()> {
    let mut conn = connect(db_path)?;
    let tx = conn.transaction()?;
    let s = &detail.summary;
    let mut compact = detail.clone();
    compact.messages.clear();
    compact.tool_calls.clear();
    compact.raw_data.clear();
    tx.execute("INSERT INTO sessions(id,agent_type,title,created_at,updated_at,working_directory,git_repository,source_path,detail_json,parse_warning) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10) ON CONFLICT(id) DO UPDATE SET title=excluded.title,updated_at=excluded.updated_at,working_directory=excluded.working_directory,git_repository=excluded.git_repository,source_path=excluded.source_path,detail_json=excluded.detail_json,parse_warning=excluded.parse_warning", params![s.id,"codex",s.title,s.created_at,s.updated_at,s.working_directory,s.git_repository,s.source_path,serde_json::to_string(&compact)?,s.parse_warning])?;
    tx.execute(
        "DELETE FROM session_messages WHERE session_id=?1",
        params![s.id],
    )?;
    tx.execute(
        "DELETE FROM session_tool_calls WHERE session_id=?1",
        params![s.id],
    )?;
    tx.execute(
        "DELETE FROM file_changes WHERE source_session_id=?1",
        params![s.id],
    )?;
    for message in &detail.messages {
        let indexed_id = format!("{}:{}", s.id, message.id);
        tx.execute("INSERT INTO session_messages(id,session_id,role,content,timestamp) VALUES(?1,?2,?3,?4,?5)", params![indexed_id,s.id,format!("{:?}", message.role).to_lowercase(),message.content,message.timestamp])?;
    }
    for tool in &detail.tool_calls {
        let indexed_id = format!("{}:{}", s.id, tool.id);
        tx.execute("INSERT INTO session_tool_calls(id,session_id,name,arguments,status,output,timestamp) VALUES(?1,?2,?3,?4,?5,?6,?7)", params![indexed_id,s.id,tool.name,tool.arguments,format!("{:?}", tool.status).to_lowercase(),tool.output,tool.timestamp])?;
    }
    for (index, path) in detail.changed_files.iter().enumerate() {
        tx.execute(
            "INSERT INTO file_changes(id,source_session_id,path,change_type,created_at) VALUES(?1,?2,?3,'changed',?4)",
            params![format!("{}:file:{index}", s.id), s.id, path, s.updated_at],
        )?;
    }
    tx.commit()?;
    Ok(())
}

const SESSION_SUMMARY_SELECT: &str = "SELECT s.id,s.title,s.agent_type,s.created_at,s.updated_at,s.working_directory,s.git_repository,(SELECT COUNT(*) FROM session_messages m WHERE m.session_id=s.id),(SELECT COUNT(*) FROM session_tool_calls t WHERE t.session_id=s.id),EXISTS(SELECT 1 FROM file_changes f WHERE f.source_session_id=s.id),EXISTS(SELECT 1 FROM session_messages m WHERE m.session_id=s.id),s.source_path,s.parse_warning,COALESCE(json_extract(ss.raw_metadata,'$.clientKind'),'unknown'),(SELECT pb.project_id FROM project_bindings pb WHERE pb.binding_type='source_session' AND pb.binding_id=s.id ORDER BY pb.created_at DESC LIMIT 1),(SELECT p.name FROM project_bindings pb JOIN projects p ON p.id=pb.project_id WHERE pb.binding_type='source_session' AND pb.binding_id=s.id ORDER BY pb.created_at DESC LIMIT 1),(SELECT um.content FROM session_messages um WHERE um.session_id=s.id AND um.role='user' AND ltrim(um.content) NOT LIKE '<recommended_plugins>%' AND ltrim(um.content) NOT LIKE '<environment_context>%' AND ltrim(um.content) NOT LIKE '<app-context>%' AND ltrim(um.content) NOT LIKE '<permissions%' AND ltrim(um.content) NOT LIKE '<collaboration_mode>%' AND ltrim(um.content) NOT LIKE '<apps_instructions>%' AND ltrim(um.content) NOT LIKE '<plugins_instructions>%' AND ltrim(um.content) NOT LIKE '<skills_instructions>%' AND ltrim(um.content) NOT LIKE '<INSTRUCTIONS>%' ORDER BY um.rowid LIMIT 1) FROM sessions s LEFT JOIN source_sessions ss ON ss.id=s.id";

fn session_summary_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SessionSummary> {
    let stored_title: String = row.get(1)?;
    let first_real_request: Option<String> = row.get(16)?;
    let title = first_real_request
        .as_deref()
        .and_then(crate::codex_adapter::human_title_from_content)
        .unwrap_or_else(|| {
            if crate::codex_adapter::title_needs_human_request(&stored_title) {
                "未命名会话".into()
            } else {
                stored_title
            }
        });
    Ok(SessionSummary {
        id: row.get(0)?,
        title,
        agent: parse_agent(row.get(2)?),
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
        working_directory: row.get(5)?,
        git_repository: row.get(6)?,
        message_count: row.get::<_, i64>(7)? as usize,
        tool_call_count: row.get::<_, i64>(8)? as usize,
        has_file_changes: row.get::<_, i64>(9)? != 0,
        can_package: row.get::<_, i64>(10)? != 0,
        source_path: row.get(11)?,
        parse_warning: row.get(12)?,
        client_kind: row.get(13)?,
        bound_project_id: row.get(14)?,
        bound_project_name: row.get(15)?,
    })
}

pub fn list_sessions(db_path: &Path) -> AppResult<Vec<SessionSummary>> {
    let conn = connect(db_path)?;
    let mut stmt = conn.prepare(&format!(
        "{SESSION_SUMMARY_SELECT} ORDER BY s.updated_at DESC"
    ))?;
    let mut summaries = stmt
        .query_map([], session_summary_from_row)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(AppError::Database)?;
    let codex_metadata = crate::codex_adapter::load_codex_thread_metadata();
    for summary in &mut summaries {
        if let Some(metadata) = codex_metadata.get(&summary.id) {
            if !metadata.title.trim().is_empty() {
                summary.title = metadata.title.clone();
            }
            if metadata.client_kind != "unknown" {
                summary.client_kind = metadata.client_kind.clone();
            }
        }
    }
    Ok(summaries)
}

pub fn get_session_summary(db_path: &Path, id: &str) -> AppResult<SessionSummary> {
    let mut summary = connect(db_path)?
        .query_row(
            &format!("{SESSION_SUMMARY_SELECT} WHERE s.id=?1"),
            params![id],
            session_summary_from_row,
        )
        .optional()?
        .ok_or_else(|| AppError::Message("找不到指定会话".into()))?;
    if let Some(metadata) = crate::codex_adapter::load_codex_thread_metadata().get(id) {
        if !metadata.title.trim().is_empty() {
            summary.title = metadata.title.clone();
        }
        if metadata.client_kind != "unknown" {
            summary.client_kind = metadata.client_kind.clone();
        }
    }
    Ok(summary)
}

pub fn earliest_user_messages(
    db_path: &Path,
    session_id: &str,
    limit: usize,
) -> AppResult<Vec<SessionMessage>> {
    let conn = connect(db_path)?;
    let mut statement = conn.prepare(
        "SELECT id,content,timestamp FROM session_messages WHERE session_id=?1 AND role='user' ORDER BY rowid LIMIT ?2",
    )?;
    let messages = statement
        .query_map(params![session_id, limit as i64], |row| {
            Ok(SessionMessage {
                id: row.get(0)?,
                role: MessageRole::User,
                content: row.get(1)?,
                timestamp: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(AppError::Database)?;
    Ok(messages)
}

pub fn get_session(db_path: &Path, id: &str) -> AppResult<SessionDetail> {
    let conn = connect(db_path)?;
    let summary = get_session_summary(db_path, id)?;
    let mut message_statement = conn.prepare(
        "SELECT id,role,content,timestamp FROM session_messages WHERE session_id=?1 ORDER BY rowid",
    )?;
    let id_prefix = format!("{id}:");
    let messages = message_statement
        .query_map(params![id], |row| {
            let role: String = row.get(1)?;
            let stored_id: String = row.get(0)?;
            Ok(SessionMessage {
                id: stored_id
                    .strip_prefix(&id_prefix)
                    .unwrap_or(&stored_id)
                    .to_owned(),
                role: match role.as_str() {
                    "user" => MessageRole::User,
                    "assistant" => MessageRole::Assistant,
                    "system" => MessageRole::System,
                    "tool" => MessageRole::Tool,
                    _ => MessageRole::Unknown,
                },
                content: row.get(2)?,
                timestamp: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut tool_statement = conn.prepare(
        "SELECT id,name,arguments,status,output,timestamp FROM session_tool_calls WHERE session_id=?1 ORDER BY rowid",
    )?;
    let tool_calls = tool_statement
        .query_map(params![id], |row| {
            let status: String = row.get(3)?;
            let stored_id: String = row.get(0)?;
            Ok(ToolCall {
                id: stored_id
                    .strip_prefix(&id_prefix)
                    .unwrap_or(&stored_id)
                    .to_owned(),
                name: row.get(1)?,
                arguments: row.get(2)?,
                status: match status.as_str() {
                    "success" => ToolStatus::Success,
                    "failed" => ToolStatus::Failed,
                    _ => ToolStatus::Unknown,
                },
                output: row.get(4)?,
                timestamp: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut file_statement =
        conn.prepare("SELECT path FROM file_changes WHERE source_session_id=?1 ORDER BY rowid")?;
    let changed_files = file_statement
        .query_map(params![id], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    let metadata: serde_json::Value = conn
        .query_row(
            "SELECT raw_metadata FROM source_sessions WHERE id=?1",
            params![id],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or_default();
    let commands = metadata
        .get("commands")
        .and_then(|value| serde_json::from_value(value.clone()).ok())
        .unwrap_or_else(|| {
            tool_calls
                .iter()
                .filter(|tool| {
                    let name = tool.name.to_ascii_lowercase();
                    name.contains("exec") || name.contains("shell") || name.contains("command")
                })
                .map(|tool| tool.arguments.clone())
                .collect()
        });
    let failed_steps = tool_calls
        .iter()
        .filter(|tool| matches!(tool.status, ToolStatus::Failed))
        .map(|tool| {
            format!(
                "{}: {}",
                tool.name,
                tool.output.as_deref().unwrap_or("工具调用失败")
            )
        })
        .collect();
    let git_state = metadata
        .get("gitState")
        .and_then(|value| serde_json::from_value(value.clone()).ok());
    let goal_summary = messages
        .iter()
        .find(|message| matches!(message.role, MessageRole::User))
        .map(|message| message.content.chars().take(320).collect())
        .unwrap_or_default();
    Ok(SessionDetail {
        summary,
        goal_summary,
        messages,
        tool_calls,
        commands,
        changed_files,
        failed_steps,
        git_state,
        raw_data: vec![],
    })
}

pub fn append_session_delta(db_path: &Path, detail: &SessionDetail) -> AppResult<()> {
    let mut conn = connect(db_path)?;
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE sessions SET title=?1,updated_at=?2,working_directory=?3,git_repository=COALESCE(?4,git_repository),parse_warning=?5 WHERE id=?6",
        params![detail.summary.title, detail.summary.updated_at, detail.summary.working_directory, detail.summary.git_repository, detail.summary.parse_warning, detail.summary.id],
    )?;
    for message in &detail.messages {
        tx.execute(
            "INSERT OR IGNORE INTO session_messages(id,session_id,role,content,timestamp) VALUES(?1,?2,?3,?4,?5)",
            params![format!("{}:{}", detail.summary.id, message.id), detail.summary.id, format!("{:?}", message.role).to_lowercase(), message.content, message.timestamp],
        )?;
    }
    for tool in &detail.tool_calls {
        tx.execute(
            "INSERT OR IGNORE INTO session_tool_calls(id,session_id,name,arguments,status,output,timestamp) VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![format!("{}:{}", detail.summary.id, tool.id), detail.summary.id, tool.name, tool.arguments, format!("{:?}", tool.status).to_lowercase(), tool.output, tool.timestamp],
        )?;
    }
    for (index, path) in detail.changed_files.iter().enumerate() {
        tx.execute(
            "INSERT OR IGNORE INTO file_changes(id,source_session_id,path,change_type,created_at) VALUES(?1,?2,?3,'changed',?4)",
            params![format!("{}:delta-file:{}:{index}", detail.summary.id, detail.summary.updated_at), detail.summary.id, path, detail.summary.updated_at],
        )?;
    }
    tx.commit()?;
    Ok(())
}

pub fn upsert_package(db_path: &Path, item: &PackageSummary) -> AppResult<()> {
    let conn = connect(db_path)?;
    conn.execute("INSERT INTO packages(id,title,source_agent,target_agent,created_at,project_path,package_path,schema_version,integrity,has_patch,security_warning_count,imported,resumed) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13) ON CONFLICT(id) DO UPDATE SET title=excluded.title,package_path=excluded.package_path,integrity=excluded.integrity,security_warning_count=excluded.security_warning_count", params![item.id,item.title,format!("{:?}",item.source_agent).to_lowercase(),format!("{:?}",item.target_agent).to_lowercase(),item.created_at,item.project_path,item.package_path,item.schema_version,item.integrity,item.has_patch,item.security_warning_count,item.imported,item.resumed])?;
    Ok(())
}

fn parse_agent(value: String) -> AgentKind {
    match value.as_str() {
        "claude" => AgentKind::Claude,
        "gemini" => AgentKind::Gemini,
        "opencode" => AgentKind::Opencode,
        "cursor" => AgentKind::Cursor,
        "copilot" => AgentKind::Copilot,
        _ => AgentKind::Codex,
    }
}

fn package_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PackageSummary> {
    Ok(PackageSummary {
        id: row.get(0)?,
        title: row.get(1)?,
        source_agent: parse_agent(row.get(2)?),
        target_agent: parse_agent(row.get(3)?),
        created_at: row.get(4)?,
        project_path: row.get(5)?,
        package_path: row.get(6)?,
        schema_version: row.get(7)?,
        integrity: row.get(8)?,
        has_patch: row.get::<_, i64>(9)? != 0,
        security_warning_count: row.get::<_, i64>(10)? as usize,
        imported: row.get::<_, i64>(11)? != 0,
        resumed: row.get::<_, i64>(12)? != 0,
    })
}

pub fn list_packages(db_path: &Path) -> AppResult<Vec<PackageSummary>> {
    let conn = connect(db_path)?;
    let mut stmt=conn.prepare("SELECT id,title,source_agent,target_agent,created_at,project_path,package_path,schema_version,integrity,has_patch,security_warning_count,imported,resumed FROM packages ORDER BY created_at DESC")?;
    let rows = stmt
        .query_map([], package_from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}
pub fn get_package_summary(db_path: &Path, id: &str) -> AppResult<PackageSummary> {
    let conn = connect(db_path)?;
    conn.query_row("SELECT id,title,source_agent,target_agent,created_at,project_path,package_path,schema_version,integrity,has_patch,security_warning_count,imported,resumed FROM packages WHERE id=?1",params![id],package_from_row).optional()?.ok_or_else(||AppError::Message("找不到指定任务包".into()))
}
pub fn delete_package_record(db_path: &Path, id: &str) -> AppResult<()> {
    connect(db_path)?.execute("DELETE FROM packages WHERE id=?1", params![id])?;
    Ok(())
}
pub fn mark_resumed(db_path: &Path, id: &str) -> AppResult<()> {
    connect(db_path)?.execute("UPDATE packages SET resumed=1 WHERE id=?1", params![id])?;
    Ok(())
}

pub fn dashboard(db_path: &Path) -> AppResult<DashboardStats> {
    let conn = connect(db_path)?;
    let session_count: i64 = conn.query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))?;
    let package_count: i64 = conn.query_row("SELECT COUNT(*) FROM packages", [], |r| r.get(0))?;
    let imported: i64 =
        conn.query_row("SELECT COUNT(*) FROM packages WHERE imported=1", [], |r| {
            r.get(0)
        })?;
    let last_scan:Option<String>=conn.query_row("SELECT completed_at FROM scan_jobs WHERE status='completed' ORDER BY completed_at DESC LIMIT 1",[],|r|r.get(0)).optional()?;
    let mut recent = list_packages(db_path)?;
    recent.truncate(5);
    Ok(DashboardStats {
        session_count: session_count as usize,
        package_count: package_count as usize,
        imported_package_count: imported as usize,
        detected_agents: if session_count > 0 {
            vec![AgentKind::Codex]
        } else {
            vec![]
        },
        last_scan_at: last_scan,
        recent_packages: recent,
        database_path: db_path.to_string_lossy().into_owned(),
    })
}
pub fn start_scan(db_path: &Path, id: &str) -> AppResult<()> {
    connect(db_path)?.execute(
        "INSERT INTO scan_jobs(id,started_at,status) VALUES(?1,?2,'running')",
        params![id, chrono::Utc::now().to_rfc3339()],
    )?;
    Ok(())
}
pub fn finish_scan(db_path: &Path, id: &str, count: usize, error: Option<&str>) -> AppResult<()> {
    let status = if error.is_some() {
        "failed"
    } else {
        "completed"
    };
    connect(db_path)?.execute(
        "UPDATE scan_jobs SET completed_at=?1,status=?2,discovered_count=?3,error=?4 WHERE id=?5",
        params![
            chrono::Utc::now().to_rfc3339(),
            status,
            count as i64,
            error,
            id
        ],
    )?;
    Ok(())
}

pub fn schema_version(db_path: &Path) -> AppResult<i64> {
    Ok(connect(db_path)?.query_row(
        "SELECT COALESCE(MAX(version),0) FROM schema_migrations",
        [],
        |row| row.get(0),
    )?)
}

pub fn health(db_path: &Path) -> AppResult<DatabaseHealth> {
    let conn = connect(db_path)?;
    let integrity: String = conn.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    let schema_version: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version),0) FROM schema_migrations",
        [],
        |row| row.get(0),
    )?;
    let orphan_nodes: i64 = conn.query_row(
        "SELECT COUNT(*) FROM conversation_nodes n LEFT JOIN projects p ON p.id=n.project_id LEFT JOIN conversation_branches b ON b.id=n.branch_id WHERE p.id IS NULL OR b.id IS NULL",
        [],
        |row| row.get(0),
    )?;
    let invalid_bindings: i64 = conn.query_row(
        "SELECT COUNT(*) FROM project_bindings pb LEFT JOIN projects p ON p.id=pb.project_id LEFT JOIN source_sessions s ON pb.binding_type='source_session' AND s.id=pb.binding_id WHERE p.id IS NULL OR (pb.binding_type='source_session' AND s.id IS NULL)",
        [],
        |row| row.get(0),
    )?;
    Ok(DatabaseHealth {
        path: db_path.to_string_lossy().into_owned(),
        schema_version,
        integrity,
        size_bytes: fs::metadata(db_path).map(|value| value.len()).unwrap_or(0),
        orphan_nodes: orphan_nodes as usize,
        invalid_bindings: invalid_bindings as usize,
        checked_at: chrono::Utc::now().to_rfc3339(),
    })
}

pub fn backup(
    db_path: &Path,
    backup_directory: &Path,
    reason: &str,
) -> AppResult<DatabaseBackupRecord> {
    if !db_path.is_file() {
        return Err(AppError::Message("Continuum 数据库不存在".into()));
    }
    fs::create_dir_all(backup_directory)?;
    let id = uuid::Uuid::new_v4().to_string();
    let created_at = chrono::Utc::now().to_rfc3339();
    let timestamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    let target = backup_directory.join(format!("continuum-{timestamp}-{}.sqlite3", &id[..8]));
    let conn = connect(db_path)?;
    conn.execute_batch("PRAGMA wal_checkpoint(FULL);")?;
    conn.execute("VACUUM INTO ?1", params![target.to_string_lossy()])?;
    let size_bytes = fs::metadata(&target)?.len();
    let sha256 = filesystem::sha256_file(&target)?;
    let schema_version = schema_version(db_path)?;
    conn.execute(
        "INSERT INTO database_backups(id,path,reason,schema_version,size_bytes,sha256,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",
        params![id, target.to_string_lossy(), reason, schema_version, size_bytes as i64, sha256, created_at],
    )?;
    Ok(DatabaseBackupRecord {
        id,
        path: target.to_string_lossy().into_owned(),
        reason: reason.into(),
        schema_version,
        size_bytes,
        sha256,
        created_at,
        restored_at: None,
    })
}

fn validate_backup(path: &Path) -> AppResult<()> {
    if !path.is_file() {
        return Err(AppError::Message("数据库备份文件不存在".into()));
    }
    let conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let integrity: String = conn.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    if integrity != "ok" {
        return Err(AppError::Message(format!(
            "数据库备份完整性检查失败：{integrity}"
        )));
    }
    let migrations: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='schema_migrations'",
        [],
        |row| row.get(0),
    )?;
    if migrations == 0 {
        return Err(AppError::Message(
            "所选文件不是 Continuum 数据库备份".into(),
        ));
    }
    Ok(())
}

pub fn restore(
    db_path: &Path,
    backup_path: &Path,
    backup_directory: &Path,
) -> AppResult<DatabaseHealth> {
    validate_backup(backup_path)?;
    let safety = backup(db_path, backup_directory, "pre_restore")?;
    let restore_result = (|| -> AppResult<DatabaseHealth> {
        for suffix in ["-wal", "-shm"] {
            let sidecar = PathBuf::from(format!("{}{suffix}", db_path.to_string_lossy()));
            if sidecar.is_file() {
                fs::remove_file(sidecar)?;
            }
        }
        fs::copy(backup_path, db_path)?;
        initialize(db_path)?;
        let result = health(db_path)?;
        if result.integrity != "ok" || result.schema_version != LATEST_SCHEMA_VERSION {
            return Err(AppError::Message("恢复后的数据库验证失败".into()));
        }
        Ok(result)
    })();
    if let Err(error) = restore_result {
        fs::copy(&safety.path, db_path)?;
        initialize(db_path)?;
        return Err(error);
    }
    connect(db_path)?.execute(
        "UPDATE database_backups SET restored_at=?1 WHERE path=?2",
        params![
            chrono::Utc::now().to_rfc3339(),
            backup_path.to_string_lossy()
        ],
    )?;
    health(db_path)
}

pub fn list_backups(db_path: &Path) -> AppResult<Vec<DatabaseBackupRecord>> {
    let conn = connect(db_path)?;
    let mut statement = conn.prepare("SELECT id,path,reason,schema_version,size_bytes,sha256,created_at,restored_at FROM database_backups ORDER BY created_at DESC")?;
    let rows = statement
        .query_map([], |row| {
            Ok(DatabaseBackupRecord {
                id: row.get(0)?,
                path: row.get(1)?,
                reason: row.get(2)?,
                schema_version: row.get(3)?,
                size_bytes: row.get::<_, i64>(4)? as u64,
                sha256: row.get(5)?,
                created_at: row.get(6)?,
                restored_at: row.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn canonical_package_root(settings: &AppSettings) -> PathBuf {
    PathBuf::from(&settings.package_output_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_v2_schema_transactionally_and_creates_backup() {
        let temporary = tempfile::tempdir().unwrap();
        let db_path = temporary.path().join("continuum.sqlite3");
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch(MIGRATION_SQL).unwrap();
            conn.execute(
                "INSERT INTO schema_migrations(version,applied_at) VALUES(1,'now'),(2,'now')",
                [],
            )
            .unwrap();
        }
        initialize(&db_path).unwrap();
        assert_eq!(schema_version(&db_path).unwrap(), LATEST_SCHEMA_VERSION);
        let conn = connect(&db_path).unwrap();
        assert!(column_exists(&conn, "projects", "normalized_path").unwrap());
        assert!(column_exists(&conn, "source_sessions", "last_imported_offset").unwrap());
        assert!(column_exists(&conn, "continuations", "failure_code").unwrap());
        let app_server_tables: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('app_server_notifications','app_server_turns','app_server_items')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(app_server_tables, 3);
        let backups = list_backups(&db_path).unwrap();
        assert_eq!(backups.len(), 1);
        assert!(Path::new(&backups[0].path).is_file());
    }

    #[test]
    fn migrates_v3_notification_schema_with_one_recoverable_backup() {
        let temporary = tempfile::tempdir().unwrap();
        let db_path = temporary.path().join("continuum.sqlite3");
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch(MIGRATION_SQL).unwrap();
            conn.execute(
                "INSERT INTO schema_migrations(version,applied_at) VALUES(1,'now'),(2,'now')",
                [],
            )
            .unwrap();
            apply_v3(&conn).unwrap();
        }
        initialize(&db_path).unwrap();
        assert_eq!(schema_version(&db_path).unwrap(), LATEST_SCHEMA_VERSION);
        let conn = connect(&db_path).unwrap();
        let table_exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='app_server_items'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(table_exists, 1);
        let backups = list_backups(&db_path).unwrap();
        assert_eq!(backups.len(), 1);
        assert_eq!(backups[0].schema_version, 3);
        assert!(Path::new(&backups[0].path).is_file());
    }

    #[test]
    fn backs_up_restores_and_validates_database() {
        let temporary = tempfile::tempdir().unwrap();
        let db_path = temporary.path().join("continuum.sqlite3");
        let backup_dir = temporary.path().join("backups");
        initialize(&db_path).unwrap();
        connect(&db_path)
            .unwrap()
            .execute(
                "INSERT INTO app_settings(key,value_json,updated_at) VALUES('restore-proof','\"before\"','now')",
                [],
            )
            .unwrap();
        let saved = backup(&db_path, &backup_dir, "manual").unwrap();
        connect(&db_path)
            .unwrap()
            .execute(
                "UPDATE app_settings SET value_json='\"after\"' WHERE key='restore-proof'",
                [],
            )
            .unwrap();
        let restored = restore(&db_path, Path::new(&saved.path), &backup_dir).unwrap();
        assert_eq!(restored.integrity, "ok");
        assert_eq!(restored.schema_version, LATEST_SCHEMA_VERSION);
        let value: String = connect(&db_path)
            .unwrap()
            .query_row(
                "SELECT value_json FROM app_settings WHERE key='restore-proof'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(value, "\"before\"");
    }

    #[test]
    fn session_list_uses_normalized_columns_without_parsing_large_detail_json() {
        let temporary = tempfile::tempdir().unwrap();
        let db_path = temporary.path().join("continuum.sqlite3");
        initialize(&db_path).unwrap();
        let oversized_invalid_json = "x".repeat(2 * 1024 * 1024);
        connect(&db_path).unwrap().execute(
            "INSERT INTO sessions(id,agent_type,title,created_at,updated_at,working_directory,source_path,detail_json) VALUES('large','codex','Large session','2026-01-01T00:00:00Z','2026-01-01T00:00:01Z','C:/repo','C:/sessions/large.jsonl',?1)",
            params![oversized_invalid_json],
        ).unwrap();
        connect(&db_path).unwrap().execute(
            "INSERT INTO session_messages(id,session_id,role,content) VALUES('large:m1','large','user','hello')",
            [],
        ).unwrap();

        let summaries = list_sessions(&db_path).unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].id, "large");
        assert_eq!(summaries[0].message_count, 1);
        assert!(summaries[0].can_package);
    }
}
