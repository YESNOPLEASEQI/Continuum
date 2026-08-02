use crate::{
    database,
    error::{AppError, AppResult},
    filesystem,
    models::{MessageRole, SessionDetail, SessionMessage, ToolCall, ToolStatus},
    unified_project,
};
use chrono::{TimeZone, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, path::Path};

const MAX_TOOL_FIELD_CHARS: usize = 256 * 1024;

pub struct NotificationContext<'a> {
    pub db_path: &'a Path,
    pub continuation_id: &'a str,
    pub project_id: &'a str,
    pub working_directory: &'a str,
}

#[derive(Default)]
struct ProjectionDelta {
    messages: Vec<SessionMessage>,
    tools: Vec<ToolCall>,
    files: Vec<String>,
}

#[derive(Default)]
struct NormalizedItem {
    item_id: String,
    item_type: String,
    status: String,
    role: Option<String>,
    content: String,
    tool_name: Option<String>,
    arguments: Option<String>,
    output: Option<String>,
    files: Vec<String>,
}

struct ReconcileItem {
    item_id: String,
    item_type: String,
    role: Option<String>,
    content: String,
    tool_name: Option<String>,
    arguments: Option<String>,
    output: Option<String>,
    status: String,
    timestamp: Option<String>,
}

struct StoredItemState {
    status: String,
    role: Option<String>,
    content: String,
    tool_name: Option<String>,
    arguments: Option<String>,
    output: Option<String>,
    item_type: String,
    last_event_ms: Option<i64>,
}

struct ProjectedItem {
    status: String,
    role: Option<String>,
    content: String,
    tool_name: Option<String>,
    arguments: Option<String>,
    output: Option<String>,
    started_at: Option<String>,
    completed_at: Option<String>,
}

pub fn persist_notification(
    context: &NotificationContext<'_>,
    process_id: u32,
    message: &Value,
) -> AppResult<()> {
    if message.get("id").is_some() {
        return Ok(());
    }
    let Some(method) = message.get("method").and_then(Value::as_str) else {
        return Ok(());
    };
    // High-frequency deltas are coalesced by Codex into authoritative
    // item/completed payloads. Persisting every token/output chunk here would
    // reintroduce write amplification on long sessions.
    if is_transient_stream_notification(method) {
        return Ok(());
    }
    let params_value = message.get("params").cloned().unwrap_or(Value::Null);
    let params = &params_value;
    let thread_id = notification_thread_id(params);
    let turn_id = params
        .get("turnId")
        .and_then(Value::as_str)
        .or_else(|| params.pointer("/turn/id").and_then(Value::as_str));
    let item_id = params
        .get("itemId")
        .and_then(Value::as_str)
        .or_else(|| params.pointer("/item/id").and_then(Value::as_str));
    let emitted_at_ms = message.get("emittedAtMs").and_then(Value::as_i64);
    let now = Utc::now().to_rfc3339();

    if let Some(thread_id) = thread_id {
        let thread = params.get("thread");
        ensure_session_records(
            context.db_path,
            thread_id,
            context.continuation_id,
            context.project_id,
            context.working_directory,
            thread,
        )?;
    }

    let mut conn = database::connect(context.db_path)?;
    let tx = conn.transaction()?;
    let hash = notification_hash(process_id, message)?;
    let inserted = tx.execute(
        "INSERT OR IGNORE INTO app_server_notifications(notification_hash,process_id,continuation_id,project_id,thread_id,turn_id,item_id,method,emitted_at_ms,processed_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        params![hash, process_id as i64, context.continuation_id, context.project_id, thread_id, turn_id, item_id, method, emitted_at_ms, now],
    )?;
    if inserted == 0 {
        tx.rollback()?;
        return Ok(());
    }

    let mut delta = ProjectionDelta::default();
    match method {
        "thread/started" => {}
        "thread/status/changed" => {
            if let Some(thread_id) = thread_id {
                tx.execute(
                    "UPDATE sessions SET updated_at=?1 WHERE id=?2",
                    params![event_time(emitted_at_ms), thread_id],
                )?;
            }
        }
        "turn/started" => {
            if let (Some(thread_id), Some(turn)) = (thread_id, params.get("turn")) {
                let turn_id = turn.get("id").and_then(Value::as_str).unwrap_or_default();
                if !turn_id.is_empty() {
                    upsert_turn(&tx, thread_id, turn_id, "inProgress", turn, false)?;
                    for item in turn
                        .get("items")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                    {
                        merge_delta(
                            &mut delta,
                            persist_item(&tx, thread_id, turn_id, item, "started", emitted_at_ms)?,
                        );
                    }
                }
            }
        }
        "turn/completed" => {
            if let (Some(thread_id), Some(turn)) = (thread_id, params.get("turn")) {
                let turn_id = turn.get("id").and_then(Value::as_str).unwrap_or_default();
                if !turn_id.is_empty() {
                    let status = turn
                        .get("status")
                        .and_then(Value::as_str)
                        .unwrap_or("completed");
                    upsert_turn(&tx, thread_id, turn_id, status, turn, true)?;
                    for item in turn
                        .get("items")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                    {
                        merge_delta(
                            &mut delta,
                            persist_item(
                                &tx,
                                thread_id,
                                turn_id,
                                item,
                                "completed",
                                emitted_at_ms,
                            )?,
                        );
                    }
                    if let Some(error) = turn.get("error").filter(|value| !value.is_null()) {
                        delta.tools.push(persist_error(
                            &tx,
                            thread_id,
                            turn_id,
                            error,
                            emitted_at_ms,
                        )?);
                    }
                }
            }
        }
        "item/started" | "item/completed" => {
            if let (Some(thread_id), Some(turn_id), Some(item)) =
                (thread_id, turn_id, params.get("item"))
            {
                let lifecycle = if method == "item/completed" {
                    "completed"
                } else {
                    "started"
                };
                let lifecycle_ms = if lifecycle == "completed" {
                    params.get("completedAtMs").and_then(Value::as_i64)
                } else {
                    params.get("startedAtMs").and_then(Value::as_i64)
                }
                .or(emitted_at_ms);
                merge_delta(
                    &mut delta,
                    persist_item(&tx, thread_id, turn_id, item, lifecycle, lifecycle_ms)?,
                );
            }
        }
        "error" => {
            if let (Some(thread_id), Some(turn_id), Some(error)) =
                (thread_id, turn_id, params.get("error"))
            {
                tx.execute(
                    "INSERT INTO app_server_turns(thread_id,turn_id,status,error_json,updated_at) VALUES(?1,?2,'failed',?3,?4) ON CONFLICT(thread_id,turn_id) DO UPDATE SET status='failed',error_json=excluded.error_json,updated_at=excluded.updated_at",
                    params![thread_id, turn_id, compact_json(error), event_time(emitted_at_ms)],
                )?;
                delta.tools.push(persist_error(
                    &tx,
                    thread_id,
                    turn_id,
                    error,
                    emitted_at_ms,
                )?);
            }
        }
        _ => {}
    }
    tx.commit()?;

    if let Some(thread_id) = thread_id {
        sync_delta(context.db_path, thread_id, delta)?;
    }
    Ok(())
}

pub fn ensure_bound_session(
    db_path: &Path,
    thread_id: &str,
    continuation_id: &str,
    project_id: &str,
    working_directory: &str,
) -> AppResult<()> {
    ensure_session_records(
        db_path,
        thread_id,
        continuation_id,
        project_id,
        working_directory,
        None,
    )
}

pub fn sync_entire_session(db_path: &Path, thread_id: &str) -> AppResult<usize> {
    let detail = database::get_session(db_path, thread_id)?;
    unified_project::sync_indexed_session(db_path, &detail)
}

pub fn reconcile_jsonl_detail(db_path: &Path, detail: &mut SessionDetail) -> AppResult<()> {
    let conn = database::connect(db_path)?;
    let mut statement = conn.prepare(
        "SELECT item_id,item_type,role,content,tool_name,arguments,output,status,COALESCE(completed_at,started_at) FROM app_server_items WHERE thread_id=?1 ORDER BY COALESCE(started_at,updated_at),item_id",
    )?;
    let candidates = statement
        .query_map(params![detail.summary.id], |row| {
            Ok(ReconcileItem {
                item_id: row.get(0)?,
                item_type: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                tool_name: row.get(4)?,
                arguments: row.get(5)?,
                output: row.get(6)?,
                status: row.get(7)?,
                timestamp: row.get(8)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(statement);
    if candidates.is_empty() {
        return Ok(());
    }

    let mut used = BTreeSet::new();
    for message in &mut detail.messages {
        let role = role_name(&message.role);
        let key = comparison_key(&message.content);
        let match_index = candidates
            .iter()
            .enumerate()
            .find_map(|(index, candidate)| {
                (!used.contains(&index)
                    && candidate.role.as_deref() == Some(role)
                    && comparison_key(&candidate.content) == key)
                    .then_some(index)
            });
        if let Some(index) = match_index {
            let original_id = message.id.clone();
            message.id = candidates[index].item_id.clone();
            message.content = candidates[index].content.clone();
            message.timestamp = candidates[index]
                .timestamp
                .clone()
                .or(message.timestamp.clone());
            used.insert(index);
            conn.execute(
                "UPDATE app_server_items SET jsonl_verified=1,jsonl_source_id=?1 WHERE thread_id=?2 AND item_id=?3",
                params![original_id, detail.summary.id, message.id],
            )?;
        }
    }
    for tool in &mut detail.tool_calls {
        let match_index = candidates
            .iter()
            .enumerate()
            .find_map(|(index, candidate)| {
                (!used.contains(&index)
                    && candidate.tool_name.is_some()
                    && tool_names_match(
                        &candidate.item_type,
                        candidate.tool_name.as_deref().unwrap_or_default(),
                        &tool.name,
                    ))
                .then_some(index)
            });
        if let Some(index) = match_index {
            let original_id = tool.id.clone();
            tool.id = candidates[index].item_id.clone();
            tool.name = candidates[index]
                .tool_name
                .clone()
                .unwrap_or_else(|| tool.name.clone());
            tool.arguments = candidates[index]
                .arguments
                .clone()
                .unwrap_or_else(|| tool.arguments.clone());
            tool.output = candidates[index].output.clone().or(tool.output.clone());
            tool.status = parse_tool_status(&candidates[index].status);
            tool.timestamp = candidates[index]
                .timestamp
                .clone()
                .or(tool.timestamp.clone());
            used.insert(index);
            conn.execute(
                "UPDATE app_server_items SET jsonl_verified=1,jsonl_source_id=?1 WHERE thread_id=?2 AND item_id=?3",
                params![original_id, detail.summary.id, tool.id],
            )?;
        }
    }
    for (index, candidate) in candidates.iter().enumerate() {
        if candidate.item_type == "fileChange" {
            if let Some(changes) = candidate
                .arguments
                .as_deref()
                .and_then(|value| serde_json::from_str::<Value>(value).ok())
                .and_then(|value| value.as_array().cloned())
            {
                for path in changes.into_iter().filter_map(|change| {
                    change
                        .get("path")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                }) {
                    if !detail.changed_files.contains(&path) {
                        detail.changed_files.push(path);
                    }
                }
            }
        }
        if used.contains(&index) {
            continue;
        }
        if let Some(role) = candidate.role.as_deref() {
            if !candidate.content.trim().is_empty() {
                detail.messages.push(SessionMessage {
                    id: candidate.item_id.clone(),
                    role: parse_role(role),
                    content: candidate.content.clone(),
                    timestamp: candidate.timestamp.clone(),
                });
            }
        } else if let Some(name) = candidate.tool_name.as_deref() {
            detail.tool_calls.push(ToolCall {
                id: candidate.item_id.clone(),
                name: name.to_owned(),
                arguments: candidate.arguments.clone().unwrap_or_else(|| "{}".into()),
                status: parse_tool_status(&candidate.status),
                output: candidate.output.clone(),
                timestamp: candidate.timestamp.clone(),
            });
        }
    }
    detail.summary.message_count = detail.messages.len();
    detail.summary.tool_call_count = detail.tool_calls.len();
    detail.summary.has_file_changes = !detail.changed_files.is_empty();
    detail.summary.can_package = !detail.messages.is_empty();
    Ok(())
}

fn ensure_session_records(
    db_path: &Path,
    thread_id: &str,
    continuation_id: &str,
    project_id: &str,
    working_directory: &str,
    thread: Option<&Value>,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    let created_at = thread
        .and_then(|value| value.get("createdAt"))
        .and_then(Value::as_i64)
        .and_then(|seconds| Utc.timestamp_opt(seconds, 0).single())
        .map(|value| value.to_rfc3339())
        .unwrap_or_else(|| now.clone());
    let updated_at = thread
        .and_then(|value| value.get("updatedAt"))
        .and_then(Value::as_i64)
        .and_then(|seconds| Utc.timestamp_opt(seconds, 0).single())
        .map(|value| value.to_rfc3339())
        .unwrap_or_else(|| now.clone());
    let cwd = thread
        .and_then(|value| value.get("cwd"))
        .and_then(Value::as_str)
        .unwrap_or(working_directory);
    let title = thread
        .and_then(|value| value.get("name"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            thread
                .and_then(|value| value.get("preview"))
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
        })
        .map(|value| value.chars().take(72).collect::<String>())
        .unwrap_or_else(|| format!("Codex App Server {thread_id}"));
    let actual_path = thread
        .and_then(|value| value.get("path"))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty());
    let source_path = actual_path
        .map(str::to_owned)
        .unwrap_or_else(|| format!("app-server://{thread_id}"));
    let metadata = json!({
        "transport": "app_server",
        "continuationId": continuation_id,
        "projectId": project_id,
        "sessionId": thread.and_then(|value| value.get("sessionId")),
        "cliVersion": thread.and_then(|value| value.get("cliVersion")),
    });
    let normalized_cwd = filesystem::normalize_path_key(Path::new(cwd));
    let conn = database::connect(db_path)?;
    conn.execute(
        "INSERT INTO sessions(id,agent_type,title,created_at,updated_at,working_directory,git_repository,source_path,detail_json,parse_warning) VALUES(?1,'codex',?2,?3,?4,?5,NULL,?6,?7,NULL) ON CONFLICT(id) DO UPDATE SET title=CASE WHEN sessions.title LIKE 'Codex App Server %' THEN excluded.title ELSE sessions.title END,created_at=MIN(sessions.created_at,excluded.created_at),updated_at=MAX(sessions.updated_at,excluded.updated_at),working_directory=COALESCE(sessions.working_directory,excluded.working_directory),source_path=CASE WHEN excluded.source_path NOT LIKE 'app-server://%' THEN excluded.source_path ELSE sessions.source_path END",
        params![thread_id, title, created_at, updated_at, cwd, source_path, metadata.to_string()],
    )?;
    conn.execute(
        "INSERT INTO source_sessions(id,agent_type,title,source_path,working_directory,created_at,updated_at,detail_json,external_session_id,session_file_path,normalized_working_directory,last_imported_offset,last_imported_line,pending_fragment,file_hash,bound_project_id,bound_branch_id,status,raw_metadata,file_created_at,file_modified_at) VALUES(?1,'codex',?2,?3,?4,?5,?6,'{}',?1,?7,?8,0,0,'','',?9,(SELECT branch_id FROM continuations WHERE id=?10),'bound',?11,NULL,NULL) ON CONFLICT(id) DO UPDATE SET title=CASE WHEN source_sessions.title LIKE 'Codex App Server %' THEN excluded.title ELSE source_sessions.title END,source_path=CASE WHEN excluded.source_path NOT LIKE 'app-server://%' THEN excluded.source_path ELSE source_sessions.source_path END,session_file_path=CASE WHEN excluded.session_file_path<>'' THEN excluded.session_file_path ELSE source_sessions.session_file_path END,working_directory=COALESCE(source_sessions.working_directory,excluded.working_directory),normalized_working_directory=CASE WHEN source_sessions.normalized_working_directory='' THEN excluded.normalized_working_directory ELSE source_sessions.normalized_working_directory END,created_at=MIN(source_sessions.created_at,excluded.created_at),updated_at=MAX(source_sessions.updated_at,excluded.updated_at),bound_project_id=COALESCE(source_sessions.bound_project_id,excluded.bound_project_id),bound_branch_id=COALESCE(source_sessions.bound_branch_id,excluded.bound_branch_id),status=CASE WHEN source_sessions.status='missing' THEN source_sessions.status ELSE 'bound' END,raw_metadata=excluded.raw_metadata",
        params![thread_id, title, source_path, cwd, created_at, updated_at, actual_path.unwrap_or(""), normalized_cwd, project_id, continuation_id, metadata.to_string()],
    )?;
    Ok(())
}

fn upsert_turn(
    conn: &Connection,
    thread_id: &str,
    turn_id: &str,
    status: &str,
    turn: &Value,
    completed: bool,
) -> AppResult<()> {
    let started_at = turn
        .get("startedAt")
        .and_then(Value::as_i64)
        .and_then(|value| Utc.timestamp_opt(value, 0).single())
        .map(|value| value.to_rfc3339());
    let completed_at = turn
        .get("completedAt")
        .and_then(Value::as_i64)
        .and_then(|value| Utc.timestamp_opt(value, 0).single())
        .map(|value| value.to_rfc3339())
        .or_else(|| completed.then(|| Utc::now().to_rfc3339()));
    conn.execute(
        "INSERT INTO app_server_turns(thread_id,turn_id,status,started_at,completed_at,error_json,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(thread_id,turn_id) DO UPDATE SET status=CASE WHEN app_server_turns.completed_at IS NOT NULL AND excluded.completed_at IS NULL THEN app_server_turns.status ELSE excluded.status END,started_at=COALESCE(app_server_turns.started_at,excluded.started_at),completed_at=COALESCE(excluded.completed_at,app_server_turns.completed_at),error_json=COALESCE(excluded.error_json,app_server_turns.error_json),updated_at=MAX(app_server_turns.updated_at,excluded.updated_at)",
        params![thread_id, turn_id, status, started_at, completed_at, turn.get("error").filter(|value| !value.is_null()).map(compact_json), Utc::now().to_rfc3339()],
    )?;
    Ok(())
}

fn persist_item(
    conn: &Connection,
    thread_id: &str,
    turn_id: &str,
    item: &Value,
    lifecycle: &str,
    event_ms: Option<i64>,
) -> AppResult<ProjectionDelta> {
    let normalized = normalize_item(item, lifecycle);
    if normalized.item_id.is_empty() || normalized.item_type.is_empty() {
        return Ok(ProjectionDelta::default());
    }
    let existing: Option<StoredItemState> = conn
        .query_row(
            "SELECT status,role,content,tool_name,arguments,output,item_type,last_event_ms FROM app_server_items WHERE thread_id=?1 AND item_id=?2",
            params![thread_id, normalized.item_id],
            |row| Ok(StoredItemState { status: row.get(0)?, role: row.get(1)?, content: row.get(2)?, tool_name: row.get(3)?, arguments: row.get(4)?, output: row.get(5)?, item_type: row.get(6)?, last_event_ms: row.get(7)? }),
        )
        .optional()?;
    let event_ms = event_ms.unwrap_or_else(|| Utc::now().timestamp_millis());
    let is_newer = existing
        .as_ref()
        .and_then(|value| value.last_event_ms)
        .is_none_or(|value| event_ms >= value);
    let final_event = lifecycle == "completed" && is_newer;
    let content = if final_event
        || existing
            .as_ref()
            .is_none_or(|value| value.content.is_empty())
    {
        normalized.content.clone()
    } else {
        existing
            .as_ref()
            .map(|value| value.content.clone())
            .unwrap_or_default()
    };
    let output = if final_event
        || existing
            .as_ref()
            .and_then(|value| value.output.as_ref())
            .is_none()
    {
        normalized.output.clone()
    } else {
        existing.as_ref().and_then(|value| value.output.clone())
    };
    let status = if final_event || existing.is_none() {
        normalized.status.clone()
    } else {
        existing
            .as_ref()
            .map(|value| value.status.clone())
            .unwrap_or_else(|| normalized.status.clone())
    };
    let role = normalized
        .role
        .clone()
        .or_else(|| existing.as_ref().and_then(|value| value.role.clone()));
    let tool_name = normalized
        .tool_name
        .clone()
        .or_else(|| existing.as_ref().and_then(|value| value.tool_name.clone()));
    let arguments = normalized
        .arguments
        .clone()
        .or_else(|| existing.as_ref().and_then(|value| value.arguments.clone()));
    let item_type = if normalized.item_type.is_empty() {
        existing
            .as_ref()
            .map(|value| value.item_type.clone())
            .unwrap_or_default()
    } else {
        normalized.item_type.clone()
    };
    let now = event_time(Some(event_ms));
    conn.execute(
        "INSERT INTO app_server_items(thread_id,turn_id,item_id,item_type,status,role,content,tool_name,arguments,output,started_at,completed_at,last_event_ms,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14) ON CONFLICT(thread_id,item_id) DO UPDATE SET turn_id=excluded.turn_id,item_type=excluded.item_type,status=excluded.status,role=excluded.role,content=excluded.content,tool_name=excluded.tool_name,arguments=excluded.arguments,output=excluded.output,started_at=COALESCE(app_server_items.started_at,excluded.started_at),completed_at=COALESCE(excluded.completed_at,app_server_items.completed_at),last_event_ms=MAX(COALESCE(app_server_items.last_event_ms,0),excluded.last_event_ms),updated_at=excluded.updated_at",
        params![thread_id, turn_id, normalized.item_id, item_type, status, role, content, tool_name, arguments, output, (lifecycle == "started").then(|| now.clone()), (lifecycle == "completed").then(|| now.clone()), event_ms, now],
    )?;
    project_item(conn, thread_id, &normalized.item_id, &normalized.files)
}

fn project_item(
    conn: &Connection,
    thread_id: &str,
    item_id: &str,
    files: &[String],
) -> AppResult<ProjectionDelta> {
    let row: ProjectedItem = conn.query_row(
        "SELECT status,role,content,tool_name,arguments,output,started_at,completed_at FROM app_server_items WHERE thread_id=?1 AND item_id=?2",
        params![thread_id,item_id],
        |row| Ok(ProjectedItem { status: row.get(0)?, role: row.get(1)?, content: row.get(2)?, tool_name: row.get(3)?, arguments: row.get(4)?, output: row.get(5)?, started_at: row.get(6)?, completed_at: row.get(7)? }),
    )?;
    let timestamp = row.completed_at.clone().or(row.started_at.clone());
    let mut delta = ProjectionDelta::default();
    if let Some(role) = row.role.as_deref() {
        conn.execute(
            "INSERT INTO session_messages(id,session_id,role,content,timestamp) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(id) DO UPDATE SET role=excluded.role,content=excluded.content,timestamp=COALESCE(excluded.timestamp,session_messages.timestamp)",
            params![format!("{thread_id}:{item_id}"),thread_id,role,row.content,timestamp],
        )?;
        delta.messages.push(SessionMessage {
            id: item_id.to_owned(),
            role: parse_role(role),
            content: row.content,
            timestamp,
        });
    } else if let Some(name) = row.tool_name {
        let status = parse_tool_status(&row.status);
        conn.execute(
            "INSERT INTO session_tool_calls(id,session_id,name,arguments,status,output,timestamp) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(id) DO UPDATE SET name=excluded.name,arguments=excluded.arguments,status=excluded.status,output=excluded.output,timestamp=COALESCE(excluded.timestamp,session_tool_calls.timestamp)",
            params![format!("{thread_id}:{item_id}"),thread_id,name,row.arguments.as_deref().unwrap_or("{}"),tool_status_name(&status),row.output,timestamp],
        )?;
        delta.tools.push(ToolCall {
            id: item_id.to_owned(),
            name,
            arguments: row.arguments.unwrap_or_else(|| "{}".into()),
            status,
            output: row.output,
            timestamp,
        });
    }
    for path in files {
        conn.execute(
            "INSERT INTO file_changes(id,source_session_id,path,change_type,created_at) VALUES(?1,?2,?3,'modified',?4) ON CONFLICT(id) DO UPDATE SET change_type=excluded.change_type,created_at=excluded.created_at",
            params![format!("{thread_id}:{path}"),thread_id,path,Utc::now().to_rfc3339()],
        )?;
        delta.files.push(path.clone());
    }
    Ok(delta)
}

fn persist_error(
    conn: &Connection,
    thread_id: &str,
    turn_id: &str,
    error: &Value,
    event_ms: Option<i64>,
) -> AppResult<ToolCall> {
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("Codex App Server turn failed");
    let item_id = format!("turn-error:{turn_id}:{}", short_hash(message.as_bytes()));
    let now = event_time(event_ms);
    conn.execute(
        "INSERT INTO app_server_items(thread_id,turn_id,item_id,item_type,status,content,tool_name,arguments,output,completed_at,last_event_ms,updated_at) VALUES(?1,?2,?3,'error','failed','', 'app_server_error','{}',?4,?5,?6,?5) ON CONFLICT(thread_id,item_id) DO UPDATE SET status='failed',output=excluded.output,completed_at=excluded.completed_at,last_event_ms=excluded.last_event_ms,updated_at=excluded.updated_at",
        params![thread_id,turn_id,item_id,message,now,event_ms],
    )?;
    let delta = project_item(conn, thread_id, &item_id, &[])?;
    delta
        .tools
        .into_iter()
        .next()
        .ok_or_else(|| AppError::Message("无法持久化 App Server 错误".into()))
}

fn normalize_item(item: &Value, lifecycle: &str) -> NormalizedItem {
    let item_id = item
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let item_type = item
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let source_status = item.get("status").and_then(Value::as_str);
    let status = if lifecycle == "completed" {
        source_status.unwrap_or("completed")
    } else {
        source_status.unwrap_or("inProgress")
    }
    .to_owned();
    let mut value = NormalizedItem {
        item_id,
        item_type: item_type.clone(),
        status,
        ..NormalizedItem::default()
    };
    match item_type.as_str() {
        "userMessage" => {
            value.role = Some("user".into());
            value.content = user_content(item.get("content"));
        }
        "agentMessage" => {
            value.role = Some("assistant".into());
            value.content = item
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
        }
        "commandExecution" => {
            value.tool_name = Some("command_execution".into());
            value.arguments = Some(bounded_json(
                &json!({"command":item.get("command"),"cwd":item.get("cwd"),"commandActions":item.get("commandActions")}),
            ));
            value.output = item
                .get("aggregatedOutput")
                .and_then(Value::as_str)
                .map(bounded_text);
        }
        "fileChange" => {
            value.tool_name = Some("file_change".into());
            let changes = item
                .get("changes")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();
            value.arguments = Some(bounded_json(&Value::Array(
                changes
                    .iter()
                    .map(|change| json!({"path":change.get("path"),"kind":change.get("kind")}))
                    .collect(),
            )));
            value.files = changes
                .into_iter()
                .filter_map(|change| {
                    change
                        .get("path")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                })
                .collect();
        }
        "mcpToolCall" => {
            let server = item.get("server").and_then(Value::as_str).unwrap_or("mcp");
            let tool = item.get("tool").and_then(Value::as_str).unwrap_or("tool");
            value.tool_name = Some(format!("mcp:{server}/{tool}"));
            value.arguments = Some(bounded_json(item.get("arguments").unwrap_or(&Value::Null)));
            value.output = item
                .get("result")
                .map(bounded_json)
                .or_else(|| item.get("error").map(bounded_json));
        }
        "dynamicToolCall" => {
            let namespace = item
                .get("namespace")
                .and_then(Value::as_str)
                .unwrap_or("dynamic");
            let tool = item.get("tool").and_then(Value::as_str).unwrap_or("tool");
            value.tool_name = Some(format!("{namespace}:{tool}"));
            value.arguments = Some(bounded_json(item.get("arguments").unwrap_or(&Value::Null)));
            value.output = item.get("contentItems").map(bounded_json);
        }
        "collabAgentToolCall" => {
            value.tool_name = Some(format!(
                "collaboration:{}",
                item.get("tool").and_then(Value::as_str).unwrap_or("agent")
            ));
            value.arguments = Some(bounded_json(
                &json!({"prompt":item.get("prompt"),"receiverThreadIds":item.get("receiverThreadIds")}),
            ));
            value.output = item.get("agentsStates").map(bounded_json);
        }
        "webSearch" => {
            value.tool_name = Some("web_search".into());
            value.arguments = Some(bounded_json(
                &json!({"query":item.get("query"),"action":item.get("action")}),
            ));
            value.output = item.get("results").map(bounded_json);
        }
        "imageGeneration" => {
            value.tool_name = Some("image_generation".into());
            value.arguments = Some(bounded_json(
                &json!({"revisedPrompt":item.get("revisedPrompt")}),
            ));
            value.output = item.get("result").and_then(Value::as_str).map(bounded_text);
        }
        _ => {}
    }
    value
}

fn sync_delta(db_path: &Path, thread_id: &str, delta: ProjectionDelta) -> AppResult<()> {
    if delta.messages.is_empty() && delta.tools.is_empty() && delta.files.is_empty() {
        return Ok(());
    }
    let summary = database::get_session_summary(db_path, thread_id)?;
    let detail = SessionDetail {
        summary,
        goal_summary: String::new(),
        messages: delta.messages,
        tool_calls: delta.tools,
        commands: vec![],
        changed_files: delta.files,
        failed_steps: vec![],
        git_state: None,
        raw_data: vec![],
    };
    let _ = unified_project::sync_indexed_session(db_path, &detail)?;
    Ok(())
}

fn merge_delta(target: &mut ProjectionDelta, mut source: ProjectionDelta) {
    target.messages.append(&mut source.messages);
    target.tools.append(&mut source.tools);
    target.files.append(&mut source.files);
}

fn notification_thread_id(params: &Value) -> Option<&str> {
    params
        .get("threadId")
        .and_then(Value::as_str)
        .or_else(|| params.pointer("/thread/id").and_then(Value::as_str))
}

fn is_transient_stream_notification(method: &str) -> bool {
    method.ends_with("/delta")
        || method.ends_with("Delta")
        || method.ends_with("/outputDelta")
        || matches!(
            method,
            "item/fileChange/patchUpdated"
                | "item/commandExecution/terminalInteraction"
                | "item/mcpToolCall/progress"
                | "turn/diff/updated"
                | "turn/plan/updated"
        )
}

fn notification_hash(process_id: u32, message: &Value) -> AppResult<String> {
    let mut hasher = Sha256::new();
    hasher.update(process_id.to_le_bytes());
    hasher.update(serde_json::to_vec(message)?);
    Ok(hex::encode(hasher.finalize()))
}

fn short_hash(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())[..16].to_owned()
}

fn compact_json(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "null".into())
}

fn bounded_json(value: &Value) -> String {
    bounded_text(&compact_json(value))
}

fn bounded_text(value: &str) -> String {
    let mut chars = value.chars();
    let prefix = chars
        .by_ref()
        .take(MAX_TOOL_FIELD_CHARS)
        .collect::<String>();
    if chars.next().is_some() {
        format!("{prefix}\n[Continuum truncated oversized App Server tool data]")
    } else {
        prefix
    }
}

fn event_time(millis: Option<i64>) -> String {
    millis
        .and_then(|value| Utc.timestamp_millis_opt(value).single())
        .map(|value| value.to_rfc3339())
        .unwrap_or_else(|| Utc::now().to_rfc3339())
}

fn user_content(content: Option<&Value>) -> String {
    content
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| match item.get("type").and_then(Value::as_str) {
            Some("text" | "input_text") => {
                item.get("text").and_then(Value::as_str).map(str::to_owned)
            }
            Some("image") => item
                .get("url")
                .and_then(Value::as_str)
                .map(|url| format!("[image: {url}]")),
            Some("localImage") => item
                .get("path")
                .and_then(Value::as_str)
                .map(|path| format!("[image: {path}]")),
            Some("audio") => item
                .get("url")
                .and_then(Value::as_str)
                .map(|url| format!("[audio: {url}]")),
            Some("localAudio") => item
                .get("path")
                .and_then(Value::as_str)
                .map(|path| format!("[audio: {path}]")),
            Some("skill" | "mention") => item
                .get("name")
                .and_then(Value::as_str)
                .map(|name| format!("@{name}")),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn parse_role(role: &str) -> MessageRole {
    match role {
        "user" => MessageRole::User,
        "assistant" => MessageRole::Assistant,
        "system" => MessageRole::System,
        "tool" => MessageRole::Tool,
        _ => MessageRole::Unknown,
    }
}

fn role_name(role: &MessageRole) -> &'static str {
    match role {
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        MessageRole::System => "system",
        MessageRole::Tool => "tool",
        MessageRole::Unknown => "unknown",
    }
}

fn parse_tool_status(status: &str) -> ToolStatus {
    match status {
        "completed" | "success" => ToolStatus::Success,
        "failed" | "declined" => ToolStatus::Failed,
        _ => ToolStatus::Unknown,
    }
}

fn tool_status_name(status: &ToolStatus) -> &'static str {
    match status {
        ToolStatus::Success => "success",
        ToolStatus::Failed => "failed",
        ToolStatus::Unknown => "unknown",
    }
}

fn comparison_key(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn tool_names_match(item_type: &str, app_name: &str, jsonl_name: &str) -> bool {
    if app_name == jsonl_name {
        return true;
    }
    let jsonl = jsonl_name.to_ascii_lowercase();
    match item_type {
        "commandExecution" => {
            jsonl.contains("exec") || jsonl.contains("shell") || jsonl.contains("command")
        }
        "fileChange" => jsonl.contains("patch") || jsonl.contains("file"),
        "mcpToolCall" | "dynamicToolCall" => app_name
            .split([':', '/'])
            .next_back()
            .is_some_and(|name| jsonl.contains(&name.to_ascii_lowercase())),
        "webSearch" => jsonl.contains("search") || jsonl.contains("web"),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        models::{AgentKind, CreateProjectInput},
        session_indexer,
    };
    use std::fs;

    #[test]
    fn notifications_project_immediately_and_jsonl_reuses_canonical_item_ids() {
        let temporary = tempfile::tempdir().unwrap();
        let db_path = temporary.path().join("continuum.sqlite3");
        let repo = temporary.path().join("repo");
        fs::create_dir_all(&repo).unwrap();
        database::initialize(&db_path).unwrap();
        let project = unified_project::create(
            &db_path,
            &CreateProjectInput {
                name: "Notification project".into(),
                project_path: repo.to_string_lossy().into_owned(),
                goal: "Verify App Server persistence".into(),
                constraints: vec![],
                default_agent: AgentKind::Codex,
                default_model: "default".into(),
            },
            120_000,
        )
        .unwrap();
        let project_id = project.summary.id.clone();
        let branch_id = project.summary.current_branch_id.clone();
        let repo_string = repo.to_string_lossy().into_owned();
        let continuation_id = "cont-notifications";
        database::connect(&db_path).unwrap().execute(
            "INSERT INTO continuations(id,project_id,branch_id,snapshot_id,target_agent,target_model,mode,status,bootstrap_file,launch_command,created_at,working_directory,marker,started_at) VALUES(?1,?2,?3,'snapshot','codex','default','context','listening','','',?4,?5,'marker',?4)",
            params![continuation_id,project_id,branch_id,Utc::now().to_rfc3339(),repo_string],
        ).unwrap();
        let context = NotificationContext {
            db_path: &db_path,
            continuation_id,
            project_id: &project_id,
            working_directory: &repo_string,
        };
        let session_path = temporary.path().join("thread-1.jsonl");
        persist_notification(&context, 9, &json!({
            "method":"thread/started","emittedAtMs":1000,
            "params":{"thread":{"id":"thread-1","sessionId":"session-tree","name":"Fresh thread","preview":"Continue","cwd":repo,"path":session_path,"createdAt":1,"updatedAt":1,"cliVersion":"0.146.0","ephemeral":false,"modelProvider":"openai","source":"appServer","status":{"type":"active","activeFlags":[]},"turns":[]}}
        })).unwrap();
        database::connect(&db_path).unwrap().execute(
            "INSERT INTO project_bindings(project_id,binding_type,binding_id,branch_id,created_at,metadata_json) VALUES(?1,'source_session','thread-1',?2,?3,'{}')",
            params![project_id,branch_id,Utc::now().to_rfc3339()],
        ).unwrap();
        let completed = json!({
            "method":"item/completed","emittedAtMs":3000,
            "params":{"threadId":"thread-1","turnId":"turn-1","completedAtMs":3000,"item":{"id":"item-user","type":"userMessage","content":[{"type":"text","text":"Hello from App Server"}]}}
        });
        persist_notification(&context, 9, &completed).unwrap();
        persist_notification(&context, 9, &completed).unwrap();
        persist_notification(&context, 9, &json!({
            "method":"item/agentMessage/delta","emittedAtMs":4000,
            "params":{"threadId":"thread-1","turnId":"turn-1","itemId":"item-agent","delta":"Working"}
        })).unwrap();
        persist_notification(&context, 9, &json!({
            "method":"item/completed","emittedAtMs":5000,
            "params":{"threadId":"thread-1","turnId":"turn-1","completedAtMs":5000,"item":{"id":"item-agent","type":"agentMessage","text":"Working done","phase":"final_answer"}}
        })).unwrap();
        persist_notification(&context, 9, &json!({
            "method":"item/started","emittedAtMs":3500,
            "params":{"threadId":"thread-1","turnId":"turn-1","startedAtMs":3500,"item":{"id":"item-agent","type":"agentMessage","text":""}}
        })).unwrap();
        persist_notification(&context, 9, &json!({
            "method":"item/completed","emittedAtMs":6000,
            "params":{"threadId":"thread-1","turnId":"turn-1","completedAtMs":6000,"item":{"id":"item-command","type":"commandExecution","command":"cargo test","cwd":repo,"status":"completed","aggregatedOutput":"ok","exitCode":0}}
        })).unwrap();
        persist_notification(&context, 9, &json!({
            "method":"item/completed","emittedAtMs":7000,
            "params":{"threadId":"thread-1","turnId":"turn-1","completedAtMs":7000,"item":{"id":"item-file","type":"fileChange","status":"completed","changes":[{"path":"src/main.rs","kind":"update","diff":"ignored compactly"}]}}
        })).unwrap();
        let detail = database::get_session(&db_path, "thread-1").unwrap();
        assert_eq!(detail.messages.len(), 2);
        assert_eq!(detail.messages[1].content, "Working done");
        assert_eq!(detail.tool_calls.len(), 2);
        assert_eq!(detail.changed_files, vec!["src/main.rs"]);
        let notification_count: i64 = database::connect(&db_path)
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM app_server_notifications WHERE thread_id='thread-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(notification_count, 6);

        fs::write(&session_path, format!(
            "{{\"type\":\"session_meta\",\"payload\":{{\"id\":\"thread-1\",\"cwd\":{},\"timestamp\":\"1970-01-01T00:00:01Z\"}}}}\n{{\"type\":\"response_item\",\"payload\":{{\"role\":\"user\",\"content\":[{{\"text\":\"Hello from App Server\"}}]}}}}\n{{\"type\":\"response_item\",\"payload\":{{\"role\":\"assistant\",\"content\":[{{\"text\":\"Working done\"}}]}}}}\n",
            serde_json::to_string(&repo.to_string_lossy()).unwrap()
        )).unwrap();
        session_indexer::full_index_file(&db_path, &session_path, false).unwrap();
        let reconciled = database::get_session(&db_path, "thread-1").unwrap();
        assert_eq!(reconciled.messages.len(), 2);
        assert_eq!(reconciled.messages[0].id, "item-user");
        assert_eq!(reconciled.messages[1].id, "item-agent");
        assert_eq!(reconciled.tool_calls.len(), 2);
        assert_eq!(reconciled.changed_files, vec!["src/main.rs"]);
        let message_nodes: i64 = database::connect(&db_path).unwrap().query_row(
            "SELECT COUNT(*) FROM conversation_nodes WHERE source_session_id='thread-1' AND node_type='message'", [], |row| row.get(0)
        ).unwrap();
        assert_eq!(message_nodes, 2);
    }
}
