use crate::{
    agent_adapters::AgentAdapter,
    app_server_persistence,
    codex_adapter::CodexAdapter,
    database,
    error::{AppError, AppResult},
    filesystem, git_inspector,
    models::*,
    security_scanner, unified_project,
};
use chrono::{DateTime, Utc};
use rusqlite::params;
#[cfg(test)]
use rusqlite::OptionalExtension;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{BufRead, BufReader, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    thread,
    time::{Duration, SystemTime},
};

fn timestamp(value: SystemTime) -> String {
    DateTime::<Utc>::from(value).to_rfc3339()
}

fn file_times(path: &Path) -> (Option<String>, Option<String>) {
    let Ok(metadata) = fs::metadata(path) else {
        return (None, None);
    };
    (
        metadata.created().ok().map(timestamp),
        metadata.modified().ok().map(timestamp),
    )
}

fn configured_paths(settings: &AppSettings) -> Vec<PathBuf> {
    if settings.session_paths.is_empty() {
        dirs::home_dir()
            .map(|home| vec![home.join(".codex").join("sessions")])
            .unwrap_or_default()
    } else {
        settings.session_paths.iter().map(PathBuf::from).collect()
    }
}

fn discover_files(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut files = BTreeSet::new();
    for root in paths {
        if !root.is_dir() {
            continue;
        }
        for entry in walkdir::WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
        {
            if entry.file_type().is_file()
                && matches!(
                    entry.path().extension().and_then(|value| value.to_str()),
                    Some("json") | Some("jsonl")
                )
            {
                files.insert(entry.path().to_path_buf());
            }
        }
    }
    files.into_iter().collect()
}

fn trailing_fragment(path: &Path) -> AppResult<String> {
    let mut file = File::open(path)?;
    let length = file.metadata()?.len();
    if length == 0 {
        return Ok(String::new());
    }
    let read_length = length.min(64 * 1024);
    file.seek(SeekFrom::Start(length - read_length))?;
    let mut bytes = Vec::with_capacity(read_length as usize);
    file.read_to_end(&mut bytes)?;
    if bytes.last() == Some(&b'\n') {
        return Ok(String::new());
    }
    let start = bytes
        .iter()
        .rposition(|value| *value == b'\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    let candidate = String::from_utf8_lossy(&bytes[start..]).to_string();
    if candidate.trim().is_empty() || serde_json::from_str::<Value>(&candidate).is_ok() {
        Ok(String::new())
    } else {
        Ok(candidate)
    }
}

fn record_error(
    db_path: &Path,
    path: &Path,
    line: Option<usize>,
    offset: Option<u64>,
    code: &str,
    message: &str,
) -> AppResult<()> {
    database::connect(db_path)?.execute(
        "INSERT INTO session_scan_errors(session_file_path,line_number,byte_offset,error_code,message,occurred_at) VALUES(?1,?2,?3,?4,?5,?6)",
        params![path.to_string_lossy(),line.map(|value|value as i64),offset.map(|value|value as i64),code,message,Utc::now().to_rfc3339()],
    )?;
    Ok(())
}

fn store_full_cursor(db_path: &Path, detail: &SessionDetail) -> AppResult<()> {
    let path = Path::new(&detail.summary.source_path);
    let metadata = fs::metadata(path)?;
    let (file_created_at, file_modified_at) = file_times(path);
    let pending = trailing_fragment(path)?;
    let offset = metadata.len();
    let mut file = File::open(path)?;
    let mut buffer = [0_u8; 64 * 1024];
    let mut newline_count = 0;
    let mut last_byte = None;
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        newline_count += buffer[..read].iter().filter(|byte| **byte == b'\n').count();
        last_byte = buffer.get(read - 1).copied();
    }
    let completed_lines =
        newline_count + usize::from(offset > 0 && last_byte != Some(b'\n') && pending.is_empty());
    let normalized_cwd = detail
        .summary
        .working_directory
        .as_deref()
        .map(Path::new)
        .map(filesystem::normalize_path_key)
        .unwrap_or_default();
    let mut compact = detail.clone();
    compact.messages.clear();
    compact.tool_calls.clear();
    compact.raw_data.clear();
    let metadata = serde_json::json!({
        "parseWarning": detail.summary.parse_warning,
        "gitState": detail.git_state,
        "commands": detail.commands,
        "changedFiles": detail.changed_files,
        "failedSteps": detail.failed_steps,
        "clientKind": detail.summary.client_kind,
    });
    let conn = database::connect(db_path)?;
    conn.execute("INSERT INTO source_sessions(id,agent_type,title,source_path,working_directory,created_at,updated_at,detail_json,external_session_id,session_file_path,normalized_working_directory,last_imported_offset,last_imported_line,pending_fragment,file_hash,status,raw_metadata,file_created_at,file_modified_at) VALUES(?1,'codex',?2,?3,?4,?5,?6,?7,?1,?3,?8,?9,?10,?11,?12,'indexed',?13,?14,?15) ON CONFLICT(id) DO UPDATE SET title=excluded.title,source_path=excluded.source_path,working_directory=excluded.working_directory,updated_at=excluded.updated_at,detail_json=excluded.detail_json,external_session_id=excluded.external_session_id,session_file_path=excluded.session_file_path,normalized_working_directory=excluded.normalized_working_directory,last_imported_offset=excluded.last_imported_offset,last_imported_line=excluded.last_imported_line,pending_fragment=excluded.pending_fragment,file_hash=excluded.file_hash,status='indexed',raw_metadata=excluded.raw_metadata,file_created_at=excluded.file_created_at,file_modified_at=excluded.file_modified_at",params![detail.summary.id,detail.summary.title,detail.summary.source_path,detail.summary.working_directory,detail.summary.created_at,detail.summary.updated_at,serde_json::to_string(&compact)?,normalized_cwd,offset as i64,completed_lines as i64,pending,filesystem::sha256_file(path)?,metadata.to_string(),file_created_at,file_modified_at])?;
    conn.execute(
        "UPDATE session_scan_errors SET resolved_at=?1 WHERE session_file_path=?2 AND resolved_at IS NULL",
        params![Utc::now().to_rfc3339(), path.to_string_lossy()],
    )?;
    Ok(())
}

pub fn full_index_file(
    db_path: &Path,
    path: &Path,
    read_git_state: bool,
) -> AppResult<SessionDetail> {
    let adapter = CodexAdapter::new();
    let mut detail = adapter.parse_session(path)?;
    app_server_persistence::reconcile_jsonl_detail(db_path, &mut detail)?;
    if read_git_state {
        if let Some(cwd) = detail.summary.working_directory.as_deref() {
            let git = git_inspector::inspect(Path::new(cwd));
            detail.summary.git_repository = git.repository_path.clone();
            detail.git_state = Some(git);
        }
    }
    database::upsert_session(db_path, &detail)?;
    store_full_cursor(db_path, &detail)?;
    let _ = unified_project::sync_indexed_session(db_path, &detail)?;
    detail.raw_data.clear();
    Ok(detail)
}

pub fn full_scan(db_path: &Path, settings: &AppSettings) -> AppResult<Vec<SessionSummary>> {
    let job_id = uuid::Uuid::new_v4().to_string();
    database::start_scan(db_path, &job_id)?;
    let paths = configured_paths(settings);
    let files = discover_files(&paths);
    let mut indexed = 0;
    let mut errors = 0;
    for path in &files {
        match full_index_file(db_path, path, settings.read_git_state) {
            Ok(_) => indexed += 1,
            Err(error) => {
                errors += 1;
                let _ = record_error(
                    db_path,
                    path,
                    None,
                    None,
                    "file_parse_failed",
                    &error.to_string(),
                );
            }
        }
    }
    let discovered = files
        .iter()
        .map(|path| filesystem::normalize_path_key(path))
        .collect::<BTreeSet<_>>();
    let conn = database::connect(db_path)?;
    let source_paths = {
        let mut statement = conn.prepare("SELECT id,session_file_path FROM source_sessions")?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };
    for (id, source_path) in source_paths {
        if !source_path.is_empty()
            && paths
                .iter()
                .any(|root| Path::new(&source_path).starts_with(root))
            && !discovered.contains(&filesystem::normalize_path_key(Path::new(&source_path)))
        {
            conn.execute(
                "UPDATE source_sessions SET status='missing' WHERE id=?1",
                params![id],
            )?;
        }
    }
    let summary = if errors == 0 {
        None
    } else {
        Some(format!("{errors} 个会话文件解析失败；其他文件已继续索引"))
    };
    database::finish_scan(db_path, &job_id, indexed, summary.as_deref())?;
    database::list_sessions(db_path)
}

#[cfg(test)]
fn load_cursor(db_path: &Path, path: &Path) -> AppResult<Option<SourceSessionCursor>> {
    let conn = database::connect(db_path)?;
    conn.query_row("SELECT id,session_file_path,last_imported_offset,last_imported_line,pending_fragment,file_hash,file_created_at,file_modified_at,status FROM source_sessions WHERE session_file_path=?1 ORDER BY updated_at DESC LIMIT 1",params![path.to_string_lossy()],|row|Ok(SourceSessionCursor{session_id:row.get(0)?,session_file_path:row.get(1)?,last_imported_offset:row.get::<_,i64>(2)? as u64,last_imported_line:row.get::<_,i64>(3)? as usize,pending_fragment:row.get(4)?,file_hash:row.get(5)?,file_created_at:row.get(6)?,file_modified_at:row.get(7)?,status:row.get(8)?})).optional().map_err(AppError::Database)
}

fn load_cursors(db_path: &Path) -> AppResult<BTreeMap<String, SourceSessionCursor>> {
    let conn = database::connect(db_path)?;
    let mut statement = conn.prepare("SELECT id,session_file_path,last_imported_offset,last_imported_line,pending_fragment,file_hash,file_created_at,file_modified_at,status FROM source_sessions WHERE session_file_path<>''")?;
    let values = statement
        .query_map([], |row| {
            Ok(SourceSessionCursor {
                session_id: row.get(0)?,
                session_file_path: row.get(1)?,
                last_imported_offset: row.get::<_, i64>(2)? as u64,
                last_imported_line: row.get::<_, i64>(3)? as usize,
                pending_fragment: row.get(4)?,
                file_hash: row.get(5)?,
                file_created_at: row.get(6)?,
                file_modified_at: row.get(7)?,
                status: row.get(8)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(values
        .into_iter()
        .map(|cursor| {
            (
                filesystem::normalize_path_key(Path::new(&cursor.session_file_path)),
                cursor,
            )
        })
        .collect())
}

fn read_appended(path: &Path, offset: u64) -> AppResult<Vec<u8>> {
    let mut last_error = None;
    for attempt in 0..3 {
        match File::open(path).and_then(|mut file| {
            file.seek(SeekFrom::Start(offset))?;
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)?;
            Ok(bytes)
        }) {
            Ok(bytes) => return Ok(bytes),
            Err(error) => {
                last_error = Some(error);
                thread::sleep(Duration::from_millis(40 * (attempt + 1)));
            }
        }
    }
    Err(AppError::Io(
        last_error.expect("read attempt records an error"),
    ))
}

fn client_kind_from_session_meta(path: &Path) -> AppResult<String> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut first_record = String::new();
    reader.read_line(&mut first_record)?;
    let value = serde_json::from_str::<Value>(&first_record)
        .map_err(|error| AppError::Message(format!("会话元数据无效：{error}")))?;
    Ok(crate::codex_adapter::client_kind_from_raw(&[value]))
}

struct IncrementalLines {
    values: Vec<(usize, Value)>,
    errors: Vec<(usize, String)>,
    pending: String,
    completed_lines: usize,
}

fn parse_incremental_lines(pending: &str, appended: &[u8], first_line: usize) -> IncrementalLines {
    let mut text = String::with_capacity(pending.len() + appended.len());
    text.push_str(pending);
    text.push_str(&String::from_utf8_lossy(appended));
    let terminated = text.ends_with('\n');
    let mut lines = text.split('\n').map(str::to_owned).collect::<Vec<_>>();
    if terminated {
        lines.pop();
    }
    let mut pending_output = String::new();
    if !terminated {
        if let Some(last) = lines.pop() {
            if !last.trim().is_empty() {
                match serde_json::from_str::<Value>(&last) {
                    Ok(_) => lines.push(last),
                    Err(_) => pending_output = last,
                }
            }
        }
    }
    let mut values = Vec::new();
    let mut errors = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let line_number = first_line + index;
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<Value>(line) {
            Ok(value) => values.push((line_number, value)),
            Err(error) => errors.push((line_number, error.to_string())),
        }
    }
    IncrementalLines {
        values,
        errors,
        pending: pending_output,
        completed_lines: lines.len(),
    }
}

/// Returns `(is_new_session, parsed_records, inserted_timeline_nodes, parse_errors)`.
#[cfg(test)]
pub fn incremental_index_file(
    db_path: &Path,
    path: &Path,
) -> AppResult<(bool, usize, usize, usize)> {
    let cursor = load_cursor(db_path, path)?;
    incremental_index_file_with_cursor(db_path, path, cursor)
}

fn incremental_index_file_with_cursor(
    db_path: &Path,
    path: &Path,
    cursor: Option<SourceSessionCursor>,
) -> AppResult<(bool, usize, usize, usize)> {
    let Some(cursor) = cursor else {
        let detail = full_index_file(db_path, path, false)?;
        return Ok((true, detail.messages.len() + detail.tool_calls.len(), 0, 0));
    };
    let metadata = fs::metadata(path)?;
    if metadata.len() < cursor.last_imported_offset {
        let detail = full_index_file(db_path, path, false)?;
        return Ok((false, detail.messages.len() + detail.tool_calls.len(), 0, 0));
    }
    if metadata.len() == cursor.last_imported_offset {
        return Ok((false, 0, 0, 0));
    }
    let appended = read_appended(path, cursor.last_imported_offset)?;
    let parsed = parse_incremental_lines(
        &cursor.pending_fragment,
        &appended,
        cursor.last_imported_line + 1,
    );
    for (line, error) in &parsed.errors {
        record_error(
            db_path,
            path,
            Some(*line),
            Some(cursor.last_imported_offset),
            "invalid_json_line",
            error,
        )?;
    }
    let mut summary = database::get_session_summary(db_path, &cursor.session_id)?;
    let adapter = CodexAdapter::new();
    let base_record = cursor.last_imported_line;
    let mut messages = Vec::new();
    let mut tool_calls = Vec::new();
    let mut commands = Vec::new();
    let mut changed_files = Vec::new();
    let mut failed_steps = Vec::new();
    let mut inserted = 0;
    for (record_index, (_, mut value)) in parsed.values.into_iter().enumerate() {
        let _ = security_scanner::redact_value(&mut value, &path.to_string_lossy());
        let raw_index = base_record + record_index;
        let single = [value.clone()];
        for mut message in adapter.extract_messages(&single) {
            message.id = format!("message-{raw_index}");
            messages.push(message);
            inserted += 1;
        }
        for mut tool in adapter.extract_tool_calls(&single) {
            tool.id = format!("tool-{raw_index}");
            if matches!(tool.status, ToolStatus::Failed) {
                failed_steps.push(format!(
                    "{}: {}",
                    tool.name,
                    tool.output.as_deref().unwrap_or("工具调用失败")
                ));
            }
            tool_calls.push(tool);
            inserted += 1;
        }
        for command in adapter.extract_commands(&single) {
            if !commands.contains(&command) {
                commands.push(command);
            }
        }
        for file in adapter.extract_file_changes(&single) {
            if !changed_files.contains(&file) {
                changed_files.push(file);
            }
        }
    }
    let (_, modified_at) = file_times(path);
    if let Some(value) = &modified_at {
        summary.updated_at = value.clone();
    }
    if summary.client_kind == "unknown" {
        summary.client_kind =
            client_kind_from_session_meta(path).unwrap_or_else(|_| "unknown".into());
    }
    summary.message_count += messages.len();
    summary.tool_call_count += tool_calls.len();
    summary.has_file_changes |= !changed_files.is_empty();
    let mut title_messages = database::earliest_user_messages(db_path, &cursor.session_id, 100)?;
    title_messages.extend(messages.iter().cloned());
    if let Some(title) = crate::codex_adapter::human_session_title(&title_messages) {
        summary.title = title;
    } else if crate::codex_adapter::title_needs_human_request(&summary.title) {
        summary.title = "未命名会话".into();
    }
    let mut detail = SessionDetail {
        summary,
        goal_summary: String::new(),
        messages,
        tool_calls,
        commands,
        changed_files,
        failed_steps,
        git_state: None,
        raw_data: vec![],
    };
    app_server_persistence::reconcile_jsonl_detail(db_path, &mut detail)?;
    database::append_session_delta(db_path, &detail)?;
    let normalized_cwd = detail
        .summary
        .working_directory
        .as_deref()
        .map(Path::new)
        .map(filesystem::normalize_path_key)
        .unwrap_or_default();
    let next_hash = filesystem::extend_hash_chain(&cursor.file_hash, &appended);
    database::connect(db_path)?.execute("UPDATE source_sessions SET title=?1,working_directory=?2,normalized_working_directory=?3,updated_at=?4,detail_json='{}',raw_metadata=?5,last_imported_offset=?6,last_imported_line=?7,pending_fragment=?8,file_hash=?9,file_modified_at=?10,status='indexed' WHERE id=?11",params![detail.summary.title,detail.summary.working_directory,normalized_cwd,detail.summary.updated_at,serde_json::json!({"commands":detail.commands,"changedFiles":detail.changed_files,"failedSteps":detail.failed_steps,"clientKind":detail.summary.client_kind}).to_string(),metadata.len() as i64,(cursor.last_imported_line+parsed.completed_lines) as i64,parsed.pending,next_hash,modified_at,cursor.session_id])?;
    let inserted_nodes = unified_project::sync_indexed_session(db_path, &detail)?;
    Ok((false, inserted, inserted_nodes, parsed.errors.len()))
}

pub fn poll(db_path: &Path, settings: &AppSettings) -> AppResult<WatchPollResult> {
    let files = discover_files(&configured_paths(settings));
    let mut cursors = load_cursors(db_path)?;
    let mut result = WatchPollResult {
        scanned_files: files.len(),
        ..WatchPollResult::default()
    };
    for path in files {
        let key = filesystem::normalize_path_key(&path);
        match incremental_index_file_with_cursor(db_path, &path, cursors.remove(&key)) {
            Ok((is_new, parsed_records, inserted_nodes, errors)) => {
                if is_new {
                    result.new_sessions += 1;
                } else if parsed_records > 0 {
                    result.updated_sessions += 1;
                }
                result.inserted_nodes += inserted_nodes;
                result.parse_errors += errors;
            }
            Err(error) => {
                result.parse_errors += 1;
                let _ = record_error(
                    db_path,
                    &path,
                    None,
                    None,
                    "watch_read_failed",
                    &error.to_string(),
                );
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn incremental_parser_preserves_half_line_and_skips_invalid_complete_line() {
        let first = parse_incremental_lines("", b"{\"ok\":1}\n{\"half\":", 1);
        assert_eq!(first.values.len(), 1);
        assert_eq!(first.pending, "{\"half\":");
        let second = parse_incremental_lines(&first.pending, b"true}\nnot-json\n{\"ok\":2}\n", 2);
        assert_eq!(second.values.len(), 2);
        assert_eq!(second.errors.len(), 1);
        assert!(second.pending.is_empty());
    }

    #[test]
    fn duplicate_poll_reads_no_bytes_twice() {
        let temporary = tempfile::tempdir().unwrap();
        let db_path = temporary.path().join("continuum.sqlite3");
        database::initialize(&db_path).unwrap();
        let session_path = temporary.path().join("session.jsonl");
        fs::write(&session_path,"{\"type\":\"session_meta\",\"payload\":{\"id\":\"cursor-session\",\"cwd\":\"C:/repo\",\"timestamp\":\"2026-01-01T00:00:00Z\"}}\n{\"type\":\"response_item\",\"payload\":{\"role\":\"user\",\"content\":[{\"text\":\"first\"}]}}\n").unwrap();
        full_index_file(&db_path, &session_path, false).unwrap();
        assert_eq!(
            incremental_index_file(&db_path, &session_path).unwrap().1,
            0
        );
        database::connect(&db_path)
            .unwrap()
            .execute(
                "UPDATE sessions SET detail_json=?1 WHERE id='cursor-session'",
                params!["not-json".repeat(1024 * 1024)],
            )
            .unwrap();
        fs::OpenOptions::new().append(true).open(&session_path).unwrap().write_all(b"{\"type\":\"response_item\",\"payload\":{\"role\":\"assistant\",\"content\":[{\"text\":\"second\"}]}}\n").unwrap();
        assert!(incremental_index_file(&db_path, &session_path).unwrap().1 > 0);
        assert_eq!(
            incremental_index_file(&db_path, &session_path).unwrap().1,
            0
        );
    }

    #[test]
    fn incremental_repair_uses_earliest_real_request_and_session_originator() {
        let temporary = tempfile::tempdir().unwrap();
        let db_path = temporary.path().join("continuum.sqlite3");
        database::initialize(&db_path).unwrap();
        let session_path = temporary.path().join("rollout-session.jsonl");
        fs::write(
            &session_path,
            "{\"type\":\"session_meta\",\"payload\":{\"id\":\"repair-session\",\"originator\":\"Codex Desktop\",\"timestamp\":\"2026-01-01T00:00:00Z\"}}\n{\"type\":\"response_item\",\"payload\":{\"role\":\"user\",\"content\":[{\"text\":\"<recommended_plugins>injected</recommended_plugins>\"}]}}\n{\"type\":\"response_item\",\"payload\":{\"role\":\"user\",\"content\":[{\"text\":\"最早的真实请求\"}]}}\n",
        )
        .unwrap();
        full_index_file(&db_path, &session_path, false).unwrap();
        database::connect(&db_path)
            .unwrap()
            .execute(
                "UPDATE sessions SET title='rollout-session' WHERE id='repair-session'",
                [],
            )
            .unwrap();
        database::connect(&db_path)
            .unwrap()
            .execute(
                "UPDATE source_sessions SET raw_metadata='{}' WHERE id='repair-session'",
                [],
            )
            .unwrap();
        fs::OpenOptions::new()
            .append(true)
            .open(&session_path)
            .unwrap()
            .write_all(b"{\"type\":\"response_item\",\"payload\":{\"role\":\"user\",\"content\":[{\"text\":\"later follow-up\"}]}}\n")
            .unwrap();
        incremental_index_file(&db_path, &session_path).unwrap();
        let summary = database::get_session_summary(&db_path, "repair-session").unwrap();
        assert_eq!(summary.title, "最早的真实请求");
        assert_eq!(summary.client_kind, "desktop");
    }
}
