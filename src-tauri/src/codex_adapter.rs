use crate::{
    agent_adapters::AgentAdapter,
    codex_runtime,
    error::{AppError, AppResult},
    models::*,
    security_scanner,
};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;
use std::{
    collections::{BTreeSet, HashMap},
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

pub struct CodexAdapter;
impl CodexAdapter {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CodexThreadMetadata {
    pub title: String,
    pub client_kind: String,
}

fn client_kind_from_thread_source(source: &str) -> String {
    match source.trim().to_ascii_lowercase().as_str() {
        "vscode" | "desktop" | "codex desktop" => "desktop",
        "cli" | "exec" => "cli",
        _ => "unknown",
    }
    .into()
}

fn read_codex_thread_metadata(
    state_path: &Path,
) -> AppResult<HashMap<String, CodexThreadMetadata>> {
    if !state_path.is_file() {
        return Ok(HashMap::new());
    }
    let conn = Connection::open_with_flags(
        state_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let mut statement = conn.prepare(
        "SELECT id,CASE WHEN trim(COALESCE(name,''))<>'' THEN name ELSE title END,source FROM threads",
    )?;
    let rows = statement
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let title: String = row.get(1)?;
            let source: String = row.get(2)?;
            Ok((
                id,
                CodexThreadMetadata {
                    title,
                    client_kind: client_kind_from_thread_source(&source),
                },
            ))
        })?
        .collect::<Result<HashMap<_, _>, _>>()?;
    Ok(rows)
}

pub(crate) fn load_codex_thread_metadata() -> HashMap<String, CodexThreadMetadata> {
    dirs::home_dir()
        .map(|home| home.join(".codex").join("state_5.sqlite"))
        .and_then(|path| read_codex_thread_metadata(&path).ok())
        .unwrap_or_default()
}

fn nested<'a>(value: &'a Value, pointers: &[&str]) -> Option<&'a Value> {
    pointers.iter().find_map(|pointer| value.pointer(pointer))
}
fn text_from_content(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Array(items) => {
            let values = items
                .iter()
                .filter_map(|item| {
                    item.get("text")
                        .or_else(|| item.get("content"))
                        .and_then(Value::as_str)
                })
                .collect::<Vec<_>>();
            if values.is_empty() {
                None
            } else {
                Some(values.join("\n"))
            }
        }
        Value::Object(map) => map
            .get("text")
            .or_else(|| map.get("content"))
            .and_then(Value::as_str)
            .map(str::to_owned),
        _ => None,
    }
}
fn timestamp(value: &Value) -> Option<String> {
    nested(
        value,
        &[
            "/timestamp",
            "/payload/timestamp",
            "/created_at",
            "/payload/created_at",
        ],
    )
    .and_then(Value::as_str)
    .map(str::to_owned)
}
fn modified_time(path: &Path) -> String {
    let value: DateTime<Utc> = fs::metadata(path)
        .and_then(|m| m.modified())
        .unwrap_or(SystemTime::now())
        .into();
    value.to_rfc3339()
}

pub(crate) fn is_protocol_injected_user_content(content: &str) -> bool {
    let trimmed = content.trim_start();
    [
        "<recommended_plugins>",
        "<environment_context>",
        "<app-context>",
        "<permissions",
        "<collaboration_mode>",
        "<apps_instructions>",
        "<plugins_instructions>",
        "<skills_instructions>",
        "<INSTRUCTIONS>",
    ]
    .iter()
    .any(|prefix| trimmed.starts_with(prefix))
}

pub(crate) fn client_kind_from_raw(raw: &[Value]) -> String {
    let client = raw.iter().find_map(|value| {
        nested(
            value,
            &[
                "/payload/originator",
                "/originator",
                "/payload/client",
                "/client",
            ],
        )
        .and_then(Value::as_str)
    });
    match client.map(str::to_ascii_lowercase).as_deref() {
        Some(value) if value.contains("desktop") => "desktop",
        Some(value) if value.contains("cli") || value.contains("codex") => "cli",
        _ => "unknown",
    }
    .to_owned()
}

pub(crate) fn first_real_user_request(messages: &[SessionMessage]) -> Option<&str> {
    messages
        .iter()
        .filter(|message| matches!(message.role, MessageRole::User))
        .map(|message| message.content.trim())
        .find(|content| !content.is_empty() && !is_protocol_injected_user_content(content))
}

pub(crate) fn human_session_title(messages: &[SessionMessage]) -> Option<String> {
    first_real_user_request(messages).map(|request| {
        let first_line = request.lines().next().unwrap_or(request);
        truncate(first_line, 72)
    })
}

pub(crate) fn human_title_from_content(content: &str) -> Option<String> {
    let content = content.trim();
    if content.is_empty() || is_protocol_injected_user_content(content) {
        return None;
    }
    Some(truncate(content.lines().next().unwrap_or(content), 72))
}

pub(crate) fn title_needs_human_request(title: &str) -> bool {
    let title = title.trim_start();
    title.starts_with('<') || title.starts_with("rollout-") || title == "未命名会话"
}

impl AgentAdapter for CodexAdapter {
    fn kind(&self) -> AgentKind {
        AgentKind::Codex
    }
    fn detect_installation(&self) -> bool {
        codex_runtime::output_with_timeout("codex", &["--version"], Duration::from_secs(5))
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
    fn get_version(&self) -> Option<String> {
        codex_runtime::output_with_timeout("codex", &["--version"], Duration::from_secs(5))
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
    }
    fn get_capabilities(&self) -> AdapterCapabilities {
        let help = codex_runtime::output_with_timeout("codex", &["--help"], Duration::from_secs(5))
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
            .unwrap_or_default();
        AdapterCapabilities {
            native_resume: help
                .lines()
                .any(|line| line.trim_start().starts_with("resume ")),
            native_fork: help
                .lines()
                .any(|line| line.trim_start().starts_with("fork ")),
            fresh_context_launch: help.contains("-C, --cd <DIR>"),
            session_watch: !self.default_session_paths().is_empty(),
            skills_discovery: true,
            mcp_discovery: true,
            status: if help.is_empty() {
                "unavailable"
            } else {
                "available"
            }
            .into(),
        }
    }
    fn default_session_paths(&self) -> Vec<PathBuf> {
        dirs::home_dir()
            .map(|home| vec![home.join(".codex").join("sessions")])
            .unwrap_or_default()
    }
    fn scan_sessions(&self, paths: &[PathBuf]) -> AppResult<Vec<SessionDetail>> {
        let mut sessions = Vec::new();
        let mut files = BTreeSet::new();
        for root in paths {
            if !root.exists() {
                continue;
            }
            for entry in walkdir::WalkDir::new(root)
                .follow_links(false)
                .into_iter()
                .filter_map(Result::ok)
            {
                if entry.file_type().is_file()
                    && matches!(
                        entry.path().extension().and_then(|e| e.to_str()),
                        Some("jsonl") | Some("json")
                    )
                {
                    files.insert(entry.path().to_path_buf());
                }
            }
        }
        for path in files {
            match self.parse_session(&path) {
                Ok(detail) => sessions.push(detail),
                Err(error) => {
                    tracing::warn!(path=%path.display(),error=%error,"Codex session parse failed")
                }
            }
        }
        sessions.sort_by(|a, b| b.summary.updated_at.cmp(&a.summary.updated_at));
        Ok(sessions)
    }
    fn parse_session(&self, path: &Path) -> AppResult<SessionDetail> {
        let content = fs::read_to_string(path)?;
        let (mut raw, warnings) = parse_jsonl(&content);
        if raw.is_empty() {
            return Err(AppError::Message(format!(
                "{} 不包含可解析的 JSON 记录",
                path.display()
            )));
        }
        let mut findings = Vec::new();
        for value in &mut raw {
            findings.extend(security_scanner::redact_value(
                value,
                &path.to_string_lossy(),
            ));
        }
        let messages = self.extract_messages(&raw);
        let tools = self.extract_tool_calls(&raw);
        let working_directory = raw
            .iter()
            .find_map(|value| {
                nested(value, &["/payload/cwd", "/cwd", "/workspace/cwd"]).and_then(Value::as_str)
            })
            .map(str::to_owned);
        let id = raw
            .iter()
            .find_map(|value| {
                nested(value, &["/payload/id", "/session_id", "/sessionId", "/id"])
                    .and_then(Value::as_str)
            })
            .map(str::to_owned)
            .or_else(|| path.file_stem().and_then(|v| v.to_str()).map(str::to_owned))
            .ok_or_else(|| AppError::Message("会话文件没有可用 ID".into()))?;
        let created_at = raw
            .iter()
            .find_map(timestamp)
            .unwrap_or_else(|| modified_time(path));
        let updated_at = raw
            .iter()
            .rev()
            .find_map(timestamp)
            .unwrap_or_else(|| modified_time(path));
        let goal_summary = first_real_user_request(&messages)
            .map(|message| truncate(message, 320))
            .unwrap_or_default();
        let title = if goal_summary.is_empty() {
            "未命名会话".to_owned()
        } else {
            human_session_title(&messages).unwrap_or_else(|| "未命名会话".into())
        };
        let commands = self.extract_commands(&raw);
        let changed_files = self.extract_file_changes(&raw);
        let failed_steps = tools
            .iter()
            .filter(|tool| matches!(tool.status, ToolStatus::Failed))
            .map(|tool| {
                format!(
                    "{}: {}",
                    tool.name,
                    tool.output.clone().unwrap_or_else(|| "工具调用失败".into())
                )
            })
            .collect::<Vec<_>>();
        let warning = if warnings.is_empty() && findings.is_empty() {
            None
        } else {
            Some(format!(
                "跳过 {} 行无效 JSON；原始数据中脱敏 {} 项",
                warnings.len(),
                findings.len()
            ))
        };
        let summary = SessionSummary {
            id,
            title,
            agent: AgentKind::Codex,
            created_at,
            updated_at,
            working_directory: working_directory.clone(),
            git_repository: None,
            message_count: messages.len(),
            tool_call_count: tools.len(),
            has_file_changes: !changed_files.is_empty(),
            can_package: !messages.is_empty(),
            source_path: path.to_string_lossy().into_owned(),
            parse_warning: warning,
            client_kind: client_kind_from_raw(&raw),
            bound_project_id: None,
            bound_project_name: None,
        };
        Ok(SessionDetail {
            summary,
            goal_summary,
            messages,
            tool_calls: tools,
            commands,
            changed_files,
            failed_steps,
            git_state: None,
            raw_data: raw,
        })
    }
    fn extract_messages(&self, raw: &[Value]) -> Vec<SessionMessage> {
        let mut result = Vec::new();
        for (value_index, value) in raw.iter().enumerate() {
            let role =
                nested(value, &["/payload/role", "/role", "/message/role"]).and_then(Value::as_str);
            let content = nested(value, &["/payload/content", "/content", "/message/content"]);
            if let (Some(role), Some(content)) = (role, content) {
                if let Some(text) = text_from_content(content) {
                    if text.trim().is_empty() {
                        continue;
                    }
                    let role = match role {
                        "user" => MessageRole::User,
                        "assistant" => MessageRole::Assistant,
                        "system" | "developer" => MessageRole::System,
                        "tool" => MessageRole::Tool,
                        _ => MessageRole::Unknown,
                    };
                    result.push(SessionMessage {
                        id: format!("message-{value_index}"),
                        role,
                        content: text,
                        timestamp: timestamp(value),
                    });
                }
            }
        }
        result
    }
    fn extract_tool_calls(&self, raw: &[Value]) -> Vec<ToolCall> {
        let mut result = Vec::new();
        for (index, value) in raw.iter().enumerate() {
            let kind = nested(value, &["/payload/type", "/type"])
                .and_then(Value::as_str)
                .unwrap_or_default();
            let name =
                nested(value, &["/payload/name", "/name", "/tool_name"]).and_then(Value::as_str);
            if kind.contains("function_call") || kind.contains("tool_call") || name.is_some() {
                let name = name.unwrap_or("unknown_tool").to_owned();
                let args = nested(value, &["/payload/arguments", "/arguments", "/input"])
                    .map(|v| {
                        if let Some(s) = v.as_str() {
                            s.to_owned()
                        } else {
                            v.to_string()
                        }
                    })
                    .unwrap_or_else(|| "{}".into());
                let output = nested(value, &["/payload/output", "/output", "/result"]).map(|v| {
                    if let Some(s) = v.as_str() {
                        s.to_owned()
                    } else {
                        v.to_string()
                    }
                });
                let failed = output
                    .as_deref()
                    .map(|s| {
                        let lower = s.to_ascii_lowercase();
                        lower.contains("error")
                            || lower.contains("failed")
                            || lower.contains("exit code 1")
                    })
                    .unwrap_or(false);
                result.push(ToolCall {
                    id: format!("tool-{index}"),
                    name,
                    arguments: args,
                    status: if failed {
                        ToolStatus::Failed
                    } else if output.is_some() {
                        ToolStatus::Success
                    } else {
                        ToolStatus::Unknown
                    },
                    output,
                    timestamp: timestamp(value),
                });
            }
        }
        result
    }
    fn extract_file_changes(&self, raw: &[Value]) -> Vec<String> {
        let mut files = BTreeSet::new();
        for value in raw {
            let name = nested(value, &["/payload/name", "/name"])
                .and_then(Value::as_str)
                .unwrap_or_default();
            if matches!(name, "apply_patch" | "write_file" | "edit_file") {
                if let Some(args) =
                    nested(value, &["/payload/arguments", "/arguments"]).and_then(Value::as_str)
                {
                    for line in args.lines() {
                        if let Some(path) = line
                            .strip_prefix("*** Update File: ")
                            .or_else(|| line.strip_prefix("*** Add File: "))
                            .or_else(|| line.strip_prefix("*** Delete File: "))
                        {
                            files.insert(path.trim().to_owned());
                        }
                    }
                }
            }
        }
        files.into_iter().collect()
    }
    fn extract_commands(&self, raw: &[Value]) -> Vec<String> {
        self.extract_tool_calls(raw)
            .into_iter()
            .filter(|tool| {
                matches!(
                    tool.name.as_str(),
                    "exec_command" | "shell" | "bash" | "powershell"
                )
            })
            .filter_map(|tool| {
                serde_json::from_str::<Value>(&tool.arguments)
                    .ok()
                    .and_then(|value| value.get("cmd").and_then(Value::as_str).map(str::to_owned))
                    .or(Some(tool.arguments))
            })
            .collect()
    }
    fn build_resume_prompt(&self, session: &SessionDetail) -> String {
        format!("你正在接手一个由 Codex 中断的任务。\n\nGoal\n{}\n\nCurrent State\n会话共有 {} 条消息和 {} 次工具调用。\n\nWorkspace Summary\n{}",session.goal_summary,session.messages.len(),session.tool_calls.len(),session.summary.working_directory.as_deref().unwrap_or("未记录"))
    }
    fn get_skills(&self) -> Vec<PathBuf> {
        dirs::home_dir()
            .map(|home| {
                vec![
                    home.join(".codex").join("skills"),
                    home.join(".agents").join("skills"),
                ]
            })
            .unwrap_or_default()
    }
    fn get_custom_instructions(&self) -> Vec<PathBuf> {
        dirs::home_dir()
            .map(|home| vec![home.join(".codex").join("AGENTS.md")])
            .unwrap_or_default()
    }
}

pub fn parse_jsonl(content: &str) -> (Vec<Value>, Vec<String>) {
    let trimmed = content.trim();
    if trimmed.starts_with('[') {
        return match serde_json::from_str::<Vec<Value>>(trimmed) {
            Ok(values) => (values, vec![]),
            Err(error) => (vec![], vec![error.to_string()]),
        };
    }
    let mut values = Vec::new();
    let mut warnings = Vec::new();
    for (index, line) in content.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str(line) {
            Ok(value) => values.push(value),
            Err(error) => warnings.push(format!("第 {} 行：{}", index + 1, error)),
        }
    }
    (values, warnings)
}
fn truncate(value: &str, max: usize) -> String {
    let text = value.trim();
    if text.chars().count() <= max {
        text.to_owned()
    } else {
        format!("{}…", text.chars().take(max).collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_codex_jsonl() {
        let text = r#"{"type":"session_meta","payload":{"id":"s1","cwd":"C:/repo","timestamp":"2025-01-01T00:00:00Z"}}
{"type":"response_item","payload":{"role":"user","content":[{"type":"input_text","text":"Fix the parser"}]}}
{"type":"function_call","payload":{"name":"exec_command","arguments":"{\"cmd\":\"cargo test\"}"}}"#;
        let (values, warnings) = parse_jsonl(text);
        assert!(warnings.is_empty());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.jsonl");
        fs::write(&path, text).unwrap();
        let session = CodexAdapter::new().parse_session(&path).unwrap();
        assert_eq!(session.summary.id, "s1");
        assert_eq!(session.messages[0].content, "Fix the parser");
        assert_eq!(session.commands, vec!["cargo test"]);
        assert_eq!(values.len(), 3);
    }
    #[test]
    fn tolerates_invalid_jsonl() {
        let (values, warnings) = parse_jsonl("{\"ok\":true}\nnot-json\n{\"still\":true}");
        assert_eq!(values.len(), 2);
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn uses_first_real_user_request_and_detects_desktop() {
        let text = r#"{"type":"session_meta","payload":{"id":"desktop-session","originator":"Codex Desktop","timestamp":"2026-08-02T00:00:00Z"}}
{"type":"response_item","payload":{"role":"user","content":[{"text":"<recommended_plugins>injected</recommended_plugins>"}]}}
{"type":"response_item","payload":{"role":"user","content":[{"text":"<environment_context>injected</environment_context>"}]}}
{"type":"response_item","payload":{"role":"user","content":[{"text":"修复 Source Sessions 标题"}]}}"#;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("desktop.jsonl");
        fs::write(&path, text).unwrap();
        let session = CodexAdapter::new().parse_session(&path).unwrap();
        assert_eq!(session.summary.title, "修复 Source Sessions 标题");
        assert_eq!(session.goal_summary, "修复 Source Sessions 标题");
        assert_eq!(session.summary.client_kind, "desktop");
    }

    #[test]
    fn reads_exact_codex_thread_title_and_source_from_state_database() {
        let temporary = tempfile::tempdir().unwrap();
        let state_path = temporary.path().join("state_5.sqlite");
        let conn = Connection::open(&state_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE threads(id TEXT PRIMARY KEY,title TEXT NOT NULL,name TEXT,source TEXT NOT NULL);\
             INSERT INTO threads VALUES('desktop-thread','优化 Source Sessions UI',NULL,'vscode');\
             INSERT INTO threads VALUES('cli-thread','Generated title','手工标题','cli');",
        )
        .unwrap();
        drop(conn);
        let metadata = read_codex_thread_metadata(&state_path).unwrap();
        assert_eq!(metadata["desktop-thread"].title, "优化 Source Sessions UI");
        assert_eq!(metadata["desktop-thread"].client_kind, "desktop");
        assert_eq!(metadata["cli-thread"].title, "手工标题");
        assert_eq!(metadata["cli-thread"].client_kind, "cli");
    }
}
