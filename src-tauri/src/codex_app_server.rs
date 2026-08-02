use crate::{
    app_server_persistence::{self, NotificationContext},
    codex_runtime,
    error::{AppError, AppResult},
    models::{AppServerClientRequest, CodexProfile},
};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        mpsc::{self, Receiver},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

const RESPONSE_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone)]
pub struct AppServerLaunch {
    pub process_id: u32,
    pub thread_id: String,
}

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct AppServerLaunchFailure {
    pub process_id: u32,
    pub thread_id: Option<String>,
    pub message: String,
}

pub struct AppServerStartRequest<'a> {
    pub db_path: &'a Path,
    pub continuation_id: &'a str,
    pub project_id: &'a str,
    pub command: &'a str,
    pub profile: Option<&'a CodexProfile>,
    pub working_directory: &'a str,
    pub target_model: &'a str,
    pub prompt: &'a str,
}

type ThreadStartResult = Result<String, (AppError, Option<String>)>;
type ManagedWriter = Arc<Mutex<Box<dyn Write + Send>>>;

#[derive(Clone)]
struct AppServerConnection {
    stdin: ManagedWriter,
}

#[derive(Clone)]
struct PendingClientRequest {
    request: AppServerClientRequest,
    rpc_id: Value,
    process_id: u32,
}

#[derive(Default)]
pub struct AppServerManager {
    connections: Mutex<HashMap<u32, AppServerConnection>>,
    pending: Mutex<HashMap<String, PendingClientRequest>>,
}

impl AppServerManager {
    fn register_connection(&self, process_id: u32, stdin: ManagedWriter) -> AppResult<()> {
        self.connections
            .lock()
            .map_err(|_| AppError::Message("App Server 连接状态已损坏".into()))?
            .insert(process_id, AppServerConnection { stdin });
        Ok(())
    }

    fn disconnect(&self, process_id: u32) {
        if let Ok(mut connections) = self.connections.lock() {
            connections.remove(&process_id);
        }
        if let Ok(mut pending) = self.pending.lock() {
            pending.retain(|_, approval| approval.process_id != process_id);
        }
    }

    fn connection(&self, process_id: u32) -> AppResult<AppServerConnection> {
        self.connections
            .lock()
            .map_err(|_| AppError::Message("App Server 连接状态已损坏".into()))?
            .get(&process_id)
            .cloned()
            .ok_or_else(|| AppError::Message("App Server 连接已经关闭".into()))
    }

    fn write_message(&self, process_id: u32, message: Value) -> AppResult<()> {
        let connection = self.connection(process_id)?;
        let mut stdin = connection
            .stdin
            .lock()
            .map_err(|_| AppError::Message("App Server 写入通道已损坏".into()))?;
        send(&mut **stdin, message)
    }

    fn handle_server_message(
        &self,
        process_id: u32,
        continuation_id: &str,
        project_id: &str,
        message: &Value,
        persistence: Option<&NotificationContext<'_>>,
    ) {
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            return;
        };
        if let Some(context) = persistence {
            let persistence_result = if method == "thread/started" {
                message
                    .pointer("/params/thread/id")
                    .and_then(Value::as_str)
                    .map(|thread_id| {
                        ensure_fresh_thread_id(context.db_path, context.continuation_id, thread_id)
                    })
                    .transpose()
                    .map(|_| ())
                    .and_then(|_| {
                        app_server_persistence::persist_notification(context, process_id, message)
                    })
            } else {
                app_server_persistence::persist_notification(context, process_id, message)
            };
            if let Err(error) = persistence_result {
                if let Ok(conn) = crate::database::connect(context.db_path) {
                    let _ = conn.execute(
                        "INSERT INTO diagnostics_events(id,level,area,code,message,metadata_json,created_at) VALUES(?1,'error','app_server','notification_persistence_failed',?2,?3,?4)",
                        rusqlite::params![uuid::Uuid::new_v4().to_string(),error.to_string(),serde_json::json!({"continuationId":continuation_id,"projectId":project_id,"method":method}).to_string(),chrono::Utc::now().to_rfc3339()],
                    );
                }
            }
        }
        if method == "serverRequest/resolved" {
            if let Some(request_id) = message.pointer("/params/requestId") {
                if let Ok(mut pending) = self.pending.lock() {
                    pending.retain(|_, request| {
                        request.process_id != process_id || request.rpc_id != *request_id
                    });
                }
            }
            return;
        }
        let Some(rpc_id) = message.get("id") else {
            return;
        };
        if !rpc_id.is_string() && !rpc_id.is_i64() && !rpc_id.is_u64() {
            return;
        }
        if matches!(
            method,
            "item/commandExecution/requestApproval"
                | "item/fileChange/requestApproval"
                | "item/permissions/requestApproval"
                | "mcpServer/elicitation/request"
                | "item/tool/requestUserInput"
        ) {
            let _ = self.queue_client_request(
                process_id,
                continuation_id,
                project_id,
                method,
                rpc_id.clone(),
                message.get("params").unwrap_or(&Value::Null),
            );
            return;
        }
        let _ = self.write_message(
            process_id,
            json!({
                "id": rpc_id,
                "error": {
                    "code": -32601,
                    "message": format!("Continuum 尚不支持 App Server 客户端请求 {method}")
                }
            }),
        );
    }

    fn queue_client_request(
        &self,
        process_id: u32,
        continuation_id: &str,
        project_id: &str,
        method: &str,
        rpc_id: Value,
        params: &Value,
    ) -> AppResult<()> {
        let network = params.get("networkApprovalContext");
        let kind = if method == "item/permissions/requestApproval" {
            "permissions"
        } else if method == "mcpServer/elicitation/request" {
            "mcp_elicitation"
        } else if method == "item/tool/requestUserInput" {
            "tool_user_input"
        } else if network.is_some_and(Value::is_object) {
            "network"
        } else if method == "item/fileChange/requestApproval" {
            "file_change"
        } else {
            "command"
        };
        let request = AppServerClientRequest {
            id: uuid::Uuid::new_v4().to_string(),
            continuation_id: continuation_id.to_owned(),
            project_id: project_id.to_owned(),
            thread_id: string_param(params, "threadId"),
            turn_id: string_param(params, "turnId"),
            item_id: string_param(params, "itemId"),
            kind: kind.into(),
            reason: optional_string_param(params, "reason"),
            command: optional_string_param(params, "command"),
            cwd: optional_string_param(params, "cwd"),
            command_actions: params
                .get("commandActions")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
            grant_root: optional_string_param(params, "grantRoot"),
            network_host: network
                .and_then(|value| value.get("host"))
                .and_then(Value::as_str)
                .map(str::to_owned),
            network_protocol: network
                .and_then(|value| value.get("protocol"))
                .and_then(Value::as_str)
                .map(str::to_owned),
            permissions: params.get("permissions").cloned(),
            server_name: optional_string_param(params, "serverName"),
            message: optional_string_param(params, "message"),
            mode: optional_string_param(params, "mode"),
            url: optional_string_param(params, "url"),
            requested_schema: params.get("requestedSchema").cloned(),
            metadata: params.get("_meta").cloned(),
            questions: params
                .get("questions")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
            auto_resolution_ms: params.get("autoResolutionMs").and_then(Value::as_u64),
            started_at_ms: params
                .get("startedAtMs")
                .and_then(Value::as_i64)
                .unwrap_or_default(),
        };
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| AppError::Message("App Server 请求队列已损坏".into()))?;
        if pending
            .values()
            .any(|request| request.process_id == process_id && request.rpc_id == rpc_id)
        {
            return Ok(());
        }
        pending.insert(
            request.id.clone(),
            PendingClientRequest {
                request,
                rpc_id,
                process_id,
            },
        );
        Ok(())
    }

    pub fn list_requests(&self) -> AppResult<Vec<AppServerClientRequest>> {
        let mut requests = self
            .pending
            .lock()
            .map_err(|_| AppError::Message("App Server 请求队列已损坏".into()))?
            .values()
            .map(|pending| pending.request.clone())
            .collect::<Vec<_>>();
        requests.sort_by(|left, right| {
            left.started_at_ms
                .cmp(&right.started_at_ms)
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(requests)
    }

    pub fn respond(&self, request_id: &str, response: Value) -> AppResult<()> {
        let pending = self
            .pending
            .lock()
            .map_err(|_| AppError::Message("App Server 请求队列已损坏".into()))?
            .get(request_id)
            .cloned()
            .ok_or_else(|| AppError::Message("该 App Server 请求已处理或已经失效".into()))?;
        validate_response(&pending.request, &response)?;
        self.write_message(
            pending.process_id,
            json!({"id": pending.rpc_id, "result": response}),
        )?;
        self.pending
            .lock()
            .map_err(|_| AppError::Message("App Server 请求队列已损坏".into()))?
            .remove(request_id);
        Ok(())
    }
}

fn validate_response(request: &AppServerClientRequest, response: &Value) -> AppResult<()> {
    let invalid = || AppError::Message("App Server 响应与请求类型不匹配".into());
    match request.kind.as_str() {
        "command" | "file_change" | "network" => {
            let decision = response.get("decision").and_then(Value::as_str);
            if !matches!(
                decision,
                Some("accept" | "acceptForSession" | "decline" | "cancel")
            ) {
                return Err(invalid());
            }
        }
        "permissions" => {
            let Some(granted) = response
                .get("permissions")
                .filter(|value| value.is_object())
            else {
                return Err(invalid());
            };
            if !request
                .permissions
                .as_ref()
                .is_some_and(|requested| is_json_subset(granted, requested))
            {
                return Err(AppError::Message("响应包含未被请求的权限".into()));
            }
            if !matches!(
                response.get("scope").and_then(Value::as_str),
                None | Some("turn" | "session")
            ) {
                return Err(invalid());
            }
        }
        "mcp_elicitation" => {
            let action = response.get("action").and_then(Value::as_str);
            if !matches!(action, Some("accept" | "decline" | "cancel")) {
                return Err(invalid());
            }
            if action == Some("accept")
                && request.mode.as_deref() != Some("url")
                && !response.get("content").is_some_and(Value::is_object)
            {
                return Err(AppError::Message(
                    "接受 MCP 表单请求时必须提交结构化内容".into(),
                ));
            }
        }
        "tool_user_input" => {
            let Some(answers) = response.get("answers").and_then(Value::as_object) else {
                return Err(invalid());
            };
            let question_ids = request
                .questions
                .iter()
                .filter_map(|question| question.get("id").and_then(Value::as_str))
                .collect::<Vec<_>>();
            if question_ids.iter().any(|id| {
                !answers.get(*id).is_some_and(|answer| {
                    answer
                        .get("answers")
                        .and_then(Value::as_array)
                        .is_some_and(|items| items.iter().all(Value::is_string))
                })
            }) || answers
                .keys()
                .any(|id| !question_ids.contains(&id.as_str()))
            {
                return Err(invalid());
            }
        }
        _ => return Err(invalid()),
    }
    Ok(())
}

fn is_json_subset(granted: &Value, requested: &Value) -> bool {
    match (granted, requested) {
        (Value::Object(granted), Value::Object(requested)) => granted.iter().all(|(key, value)| {
            requested
                .get(key)
                .is_some_and(|requested_value| is_json_subset(value, requested_value))
        }),
        (Value::Array(granted), Value::Array(requested)) => granted
            .iter()
            .all(|value| requested.iter().any(|candidate| candidate == value)),
        _ => granted == requested,
    }
}

fn string_param(params: &Value, key: &str) -> String {
    params
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn optional_string_param(params: &Value, key: &str) -> Option<String> {
    params.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn send(stdin: &mut dyn Write, message: Value) -> AppResult<()> {
    serde_json::to_writer(&mut *stdin, &message)?;
    stdin.write_all(b"\n")?;
    stdin.flush()?;
    Ok(())
}

#[cfg(test)]
fn start_thread(
    send_message: &mut impl FnMut(Value) -> AppResult<()>,
    receiver: &Receiver<Value>,
    profile: Option<&CodexProfile>,
    working_directory: &str,
    target_model: &str,
    prompt: &str,
) -> ThreadStartResult {
    start_thread_with_timeout(
        send_message,
        receiver,
        profile,
        working_directory,
        target_model,
        prompt,
        RESPONSE_TIMEOUT,
        &mut |_| Ok(()),
    )
}

#[allow(clippy::too_many_arguments)]
fn start_thread_with_timeout(
    send_message: &mut impl FnMut(Value) -> AppResult<()>,
    receiver: &Receiver<Value>,
    profile: Option<&CodexProfile>,
    working_directory: &str,
    target_model: &str,
    prompt: &str,
    response_timeout: Duration,
    validate_thread_id: &mut impl FnMut(&str) -> AppResult<()>,
) -> ThreadStartResult {
    send_message(json!({"method":"initialize","id":1,"params":{"clientInfo":{"name":"continuum","title":"Continuum","version":env!("CARGO_PKG_VERSION")},"capabilities":{"experimentalApi":true,"mcpServerOpenaiFormElicitation":true}}}))
    .map_err(|error| (error, None))?;
    response_with_timeout(receiver, 1, response_timeout).map_err(|error| (error, None))?;
    send_message(json!({"method":"initialized","params":{}})).map_err(|error| (error, None))?;
    let mut start_params =
        json!({"cwd":working_directory,"serviceName":"continuum","ephemeral":false});
    if target_model != "default" && !target_model.trim().is_empty() {
        start_params["model"] = json!(target_model);
    } else if let Some(model) = profile.and_then(|value| value.model.as_deref()) {
        start_params["model"] = json!(model);
    }
    if let Some(profile) = profile {
        start_params["approvalPolicy"] = json!(profile.approval_mode);
        start_params["sandbox"] = json!(profile.sandbox_mode);
    }
    send_message(json!({"method":"thread/start","id":2,"params":start_params}))
        .map_err(|error| (error, None))?;
    let started =
        response_with_timeout(receiver, 2, response_timeout).map_err(|error| (error, None))?;
    let thread_id = started
        .pointer("/result/thread/id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            (
                AppError::Message("Codex App Server 未返回 thread.id".into()),
                None,
            )
        })?
        .to_owned();
    validate_thread_id(&thread_id).map_err(|error| (error, None))?;
    send_message(json!({"method":"turn/start","id":3,"params":{"threadId":thread_id,"cwd":working_directory,"input":[{"type":"text","text":prompt}]}}))
    .map_err(|error| (error, Some(thread_id.clone())))?;
    response_with_timeout(receiver, 3, response_timeout)
        .map_err(|error| (error, Some(thread_id.clone())))?;
    Ok(thread_id)
}

fn response(receiver: &Receiver<Value>, id: i64) -> AppResult<Value> {
    response_with_timeout(receiver, id, RESPONSE_TIMEOUT)
}

fn response_with_timeout(
    receiver: &Receiver<Value>,
    id: i64,
    timeout: Duration,
) -> AppResult<Value> {
    let deadline = Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(AppError::Message(format!(
                "Codex App Server 等待 id={id} 响应超时"
            )));
        }
        let message = receiver.recv_timeout(remaining).map_err(|error| {
            AppError::Message(format!("Codex App Server 等待响应超时或连接断开：{error}"))
        })?;
        if message.get("method").is_some() {
            continue;
        }
        if message.get("id").and_then(Value::as_i64) != Some(id) {
            continue;
        }
        if let Some(error) = message.get("error") {
            return Err(AppError::Message(format!(
                "Codex App Server 返回错误：{error}"
            )));
        }
        return Ok(message);
    }
}

fn ensure_fresh_thread_id(db_path: &Path, continuation_id: &str, thread_id: &str) -> AppResult<()> {
    let conn = crate::database::connect(db_path)?;
    let used_by_other_continuation: i64 = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM continuations WHERE id<>?1 AND target_session_id=?2)",
        rusqlite::params![continuation_id, thread_id],
        |row| row.get(0),
    )?;
    let mut statement = conn.prepare(
        "SELECT metadata_json FROM project_bindings WHERE binding_type='source_session' AND binding_id=?1",
    )?;
    let bindings = statement.query_map([thread_id], |row| row.get::<_, String>(0))?;
    let mut used_by_other_binding = false;
    for metadata in bindings {
        let owner = serde_json::from_str::<Value>(&metadata?)
            .ok()
            .and_then(|value| {
                value
                    .get("continuationId")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            });
        if owner.as_deref() != Some(continuation_id) {
            used_by_other_binding = true;
            break;
        }
    }
    if used_by_other_continuation != 0 || used_by_other_binding {
        return Err(AppError::Message(format!(
            "Codex App Server 返回了已使用的 thread.id {thread_id}；Fresh Continuation 必须创建全新会话"
        )));
    }
    Ok(())
}

fn terminate(mut child: Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn keep_alive(mut child: Child) {
    thread::spawn(move || {
        let _ = child.wait();
    });
}

#[derive(Clone)]
struct ReaderRelay {
    manager: Arc<AppServerManager>,
    process_id: u32,
    continuation_id: String,
    project_id: String,
    db_path: PathBuf,
    working_directory: String,
}

fn start_reader(
    stdout: impl std::io::Read + Send + 'static,
    relay: Option<ReaderRelay>,
) -> Receiver<Value> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Ok(message) = serde_json::from_str::<Value>(&line) {
                if let Some(relay) = relay.as_ref() {
                    relay.manager.handle_server_message(
                        relay.process_id,
                        &relay.continuation_id,
                        &relay.project_id,
                        &message,
                        Some(&NotificationContext {
                            db_path: &relay.db_path,
                            continuation_id: &relay.continuation_id,
                            project_id: &relay.project_id,
                            working_directory: &relay.working_directory,
                        }),
                    );
                }
                let _ = sender.send(message);
            }
        }
        if let Some(relay) = relay {
            relay.manager.disconnect(relay.process_id);
        }
    });
    receiver
}

fn configure(process: &mut Command, working_directory: &str) {
    process
        .arg("app-server")
        .arg("--listen")
        .arg("stdio://")
        .current_dir(working_directory)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
}

pub fn start_fresh(
    manager: &Arc<AppServerManager>,
    request: AppServerStartRequest<'_>,
) -> Result<AppServerLaunch, AppServerLaunchFailure> {
    start_fresh_with_timeout(manager, request, RESPONSE_TIMEOUT)
}

fn start_fresh_with_timeout(
    manager: &Arc<AppServerManager>,
    request: AppServerStartRequest<'_>,
    response_timeout: Duration,
) -> Result<AppServerLaunch, AppServerLaunchFailure> {
    if request
        .profile
        .is_some_and(|value| !value.launch_arguments.is_empty())
    {
        return Err(AppServerLaunchFailure {
            process_id: 0,
            thread_id: None,
            message: "Codex Profile 含 CLI 专用启动参数，不能通过 App Server 无损启动".into(),
        });
    }
    let mut process =
        codex_runtime::command(request.command).map_err(|error| AppServerLaunchFailure {
            process_id: 0,
            thread_id: None,
            message: error.to_string(),
        })?;
    configure(&mut process, request.working_directory);
    let mut child = process.spawn().map_err(|error| AppServerLaunchFailure {
        process_id: 0,
        thread_id: None,
        message: format!("无法启动 Codex App Server：{error}"),
    })?;
    let process_id = child.id();
    let Some(stdin) = child.stdin.take() else {
        terminate(child);
        return Err(AppServerLaunchFailure {
            process_id,
            thread_id: None,
            message: "Codex App Server stdin 不可用".into(),
        });
    };
    let Some(stdout) = child.stdout.take() else {
        terminate(child);
        return Err(AppServerLaunchFailure {
            process_id,
            thread_id: None,
            message: "Codex App Server stdout 不可用".into(),
        });
    };
    let stdin: ManagedWriter = Arc::new(Mutex::new(Box::new(stdin)));
    if let Err(error) = manager.register_connection(process_id, Arc::clone(&stdin)) {
        terminate(child);
        return Err(AppServerLaunchFailure {
            process_id,
            thread_id: None,
            message: error.to_string(),
        });
    }
    let receiver = start_reader(
        stdout,
        Some(ReaderRelay {
            manager: Arc::clone(manager),
            process_id,
            continuation_id: request.continuation_id.to_owned(),
            project_id: request.project_id.to_owned(),
            db_path: request.db_path.to_path_buf(),
            working_directory: request.working_directory.to_owned(),
        }),
    );
    let mut send_message = |message| manager.write_message(process_id, message);
    let result = start_thread_with_timeout(
        &mut send_message,
        &receiver,
        request.profile,
        request.working_directory,
        request.target_model,
        request.prompt,
        response_timeout,
        &mut |thread_id| {
            ensure_fresh_thread_id(request.db_path, request.continuation_id, thread_id)
        },
    )
    .map(|thread_id| AppServerLaunch {
        process_id,
        thread_id,
    })
    .map_err(|(error, thread_id)| AppServerLaunchFailure {
        process_id,
        thread_id,
        message: error.to_string(),
    });
    match result {
        Ok(launch) => {
            keep_alive(child);
            Ok(launch)
        }
        Err(error) => {
            manager.disconnect(process_id);
            terminate(child);
            Err(error)
        }
    }
}

pub fn probe(command: &str, working_directory: &str) -> AppResult<String> {
    let mut process = codex_runtime::command(command)?;
    configure(&mut process, working_directory);
    let mut child = process.spawn()?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| AppError::Message("Codex App Server stdin 不可用".into()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::Message("Codex App Server stdout 不可用".into()))?;
    let receiver = start_reader(stdout, None);
    send(
        &mut stdin,
        json!({"method":"initialize","id":1,"params":{"clientInfo":{"name":"continuum_probe","title":"Continuum Probe","version":env!("CARGO_PKG_VERSION")}}}),
    )?;
    let value = response(&receiver, 1);
    terminate(child);
    value.map(|message| {
        message
            .pointer("/result/userAgent")
            .and_then(Value::as_str)
            .unwrap_or("codex-app-server")
            .to_owned()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    use std::sync::OnceLock;
    use std::{fs, io::Cursor};

    #[cfg(windows)]
    fn fake_process_test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[cfg(windows)]
    fn fake_server_command(temp: &tempfile::TempDir, mode: &str) -> (String, PathBuf) {
        let script = temp.path().join("fake-app-server.ps1");
        let log = temp.path().join(format!("{mode}.jsonl"));
        fs::write(&log, "").unwrap();
        fs::write(
            &script,
            r#"param([string]$Mode, [string]$LogPath)
$ErrorActionPreference = 'Stop'
function Read-Request {
  $line = [Console]::In.ReadLine()
  if ($null -ne $line) { Add-Content -LiteralPath $LogPath -Value $line -Encoding utf8 }
  return $line
}
function Write-Json([string]$Json) {
  [Console]::Out.WriteLine($Json)
  [Console]::Out.Flush()
}
$null = Read-Request
if ($Mode -eq 'exit') { exit 17 }
if ($Mode -eq 'timeout') { Start-Sleep -Seconds 5; exit 0 }
if ($Mode -eq 'initialize-error') {
  Write-Json '{"id":1,"error":{"code":-32000,"message":"fake initialize rejected"}}'
  exit 0
}
Write-Json '{"id":91,"result":{"ignored":true}}'
Write-Json '{"id":1,"result":{"userAgent":"continuum-fake"}}'
$null = Read-Request
$null = Read-Request
$threadId = if ($Mode -eq 'duplicate') { 'old-thread' } else { 'fake-thread' }
if ($Mode -eq 'duplicate') {
  Write-Json '{"method":"thread/started","params":{"thread":{"id":"old-thread","cwd":"C:\\old"}}}'
}
Write-Json '{"id":92,"result":{"ignored":true}}'
Write-Json ('{"id":2,"result":{"thread":{"id":"' + $threadId + '"}}}')
$turn = Read-Request
if ($null -ne $turn) {
  Write-Json '{"id":93,"result":{"ignored":true}}'
  Write-Json '{"id":3,"result":{"turn":{"id":"fake-turn"}}}'
}
"#,
        )
        .unwrap();
        (
            format!(
                "\"{}\" {} \"{}\"",
                script.to_string_lossy(),
                mode,
                log.to_string_lossy()
            ),
            log,
        )
    }

    #[cfg(windows)]
    fn run_fake_server(
        temp: &tempfile::TempDir,
        mode: &str,
        timeout: Duration,
    ) -> Result<AppServerLaunch, AppServerLaunchFailure> {
        let db_path = temp.path().join("continuum.sqlite3");
        crate::database::initialize(&db_path).unwrap();
        let (command, _) = fake_server_command(temp, mode);
        start_fresh_with_timeout(
            &Arc::new(AppServerManager::default()),
            AppServerStartRequest {
                db_path: &db_path,
                continuation_id: "fake-continuation",
                project_id: "fake-project",
                command: &command,
                profile: None,
                working_directory: temp.path().to_string_lossy().as_ref(),
                target_model: "default",
                prompt: "CONTINUATION_ID=fake-continuation",
            },
            timeout,
        )
    }

    #[test]
    fn response_skips_notifications_and_returns_matching_id() {
        let (sender, receiver) = mpsc::channel();
        sender.send(json!({"method":"thread/started"})).unwrap();
        sender
            .send(json!({
                "id": 2,
                "method": "item/commandExecution/requestApproval",
                "params": {}
            }))
            .unwrap();
        sender
            .send(json!({"id":2,"result":{"thread":{"id":"thread-1"}}}))
            .unwrap();
        let value = response(&receiver, 2).unwrap();
        assert_eq!(
            value.pointer("/result/thread/id").and_then(Value::as_str),
            Some("thread-1")
        );
    }

    #[test]
    fn start_thread_performs_initialize_start_and_turn_sequence() {
        let (sender, receiver) = mpsc::channel();
        sender
            .send(json!({"id":1,"result":{"userAgent":"fake"}}))
            .unwrap();
        sender
            .send(json!({"id":2,"result":{"thread":{"id":"thread-2"}}}))
            .unwrap();
        sender
            .send(json!({"id":3,"result":{"turn":{"id":"turn-1"}}}))
            .unwrap();
        let mut output = Cursor::new(Vec::new());
        let thread_id = {
            let mut send_message = |message| send(&mut output, message);
            start_thread(
                &mut send_message,
                &receiver,
                None,
                "C:\\repo",
                "default",
                "CONTINUATION_ID=abc",
            )
            .unwrap()
        };
        assert_eq!(thread_id, "thread-2");
        let lines = String::from_utf8(output.into_inner()).unwrap();
        assert!(lines.contains("\"method\":\"initialize\""));
        assert!(lines.contains("\"experimentalApi\":true"));
        assert!(lines.contains("\"mcpServerOpenaiFormElicitation\":true"));
        assert!(lines.contains("\"method\":\"thread/start\""));
        assert!(lines.contains("\"method\":\"turn/start\""));
        assert!(lines.contains("CONTINUATION_ID=abc"));
    }

    #[test]
    fn start_thread_preserves_thread_id_when_initial_turn_fails() {
        let (sender, receiver) = mpsc::channel();
        sender
            .send(json!({"id":1,"result":{"userAgent":"fake"}}))
            .unwrap();
        sender
            .send(json!({"id":2,"result":{"thread":{"id":"partial-thread"}}}))
            .unwrap();
        sender
            .send(json!({"id":3,"error":{"code":-1,"message":"turn rejected"}}))
            .unwrap();
        let mut output = Cursor::new(Vec::new());
        let mut send_message = |message| send(&mut output, message);
        let (_, thread_id) = start_thread(
            &mut send_message,
            &receiver,
            None,
            "C:\\repo",
            "default",
            "CONTINUATION_ID=partial",
        )
        .unwrap_err();
        assert_eq!(thread_id.as_deref(), Some("partial-thread"));
    }

    #[cfg(windows)]
    #[test]
    fn fake_app_server_accepts_out_of_order_responses() {
        let _guard = fake_process_test_lock().lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let launch = run_fake_server(&temp, "out-of-order", Duration::from_secs(2)).unwrap();
        assert_eq!(launch.thread_id, "fake-thread");
        let log = fs::read_to_string(temp.path().join("out-of-order.jsonl")).unwrap();
        assert!(log.contains("\"method\":\"initialize\""));
        assert!(log.contains("\"method\":\"thread/start\""));
        assert!(log.contains("\"method\":\"turn/start\""));
    }

    #[cfg(windows)]
    #[test]
    fn fake_app_server_surfaces_protocol_errors_and_early_exit() {
        let _guard = fake_process_test_lock().lock().unwrap();
        let error_temp = tempfile::tempdir().unwrap();
        let protocol_error =
            run_fake_server(&error_temp, "initialize-error", Duration::from_secs(2)).unwrap_err();
        assert!(protocol_error.message.contains("fake initialize rejected"));
        assert!(protocol_error.thread_id.is_none());

        let exit_temp = tempfile::tempdir().unwrap();
        let exit_error = run_fake_server(&exit_temp, "exit", Duration::from_secs(2)).unwrap_err();
        assert!(exit_error.message.contains("连接断开"));
        assert!(exit_error.thread_id.is_none());
    }

    #[cfg(windows)]
    #[test]
    fn fake_app_server_timeout_is_bounded_and_terminates_the_child() {
        let _guard = fake_process_test_lock().lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let started = Instant::now();
        let error = run_fake_server(&temp, "timeout", Duration::from_millis(150)).unwrap_err();
        assert!(error.message.contains("超时"));
        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(error.process_id > 0);
    }

    #[cfg(windows)]
    #[test]
    fn fake_app_server_rejects_a_reused_thread_before_starting_a_turn() {
        let _guard = fake_process_test_lock().lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("continuum.sqlite3");
        crate::database::initialize(&db_path).unwrap();
        let conn = crate::database::connect(&db_path).unwrap();
        conn.execute(
            "INSERT INTO projects(id,name,project_path,goal,constraints_json,default_agent,default_model,current_branch_id,created_at,updated_at) VALUES('old-project','Old project','C:\\old','', '[]','codex','default','old-branch','2026-08-02T00:00:00Z','2026-08-02T00:00:00Z')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO project_bindings(project_id,binding_type,binding_id,created_at,metadata_json) VALUES('old-project','source_session','old-thread','2026-08-02T00:00:00Z','{}')",
            [],
        )
        .unwrap();
        drop(conn);
        let (command, log_path) = fake_server_command(&temp, "duplicate");
        let error = start_fresh_with_timeout(
            &Arc::new(AppServerManager::default()),
            AppServerStartRequest {
                db_path: &db_path,
                continuation_id: "new-continuation",
                project_id: "new-project",
                command: &command,
                profile: None,
                working_directory: temp.path().to_string_lossy().as_ref(),
                target_model: "default",
                prompt: "CONTINUATION_ID=new-continuation",
            },
            Duration::from_secs(2),
        )
        .unwrap_err();
        assert!(error.message.contains("已使用的 thread.id old-thread"));
        assert!(error.thread_id.is_none());
        let log = fs::read_to_string(log_path).unwrap();
        assert!(!log.contains("\"method\":\"turn/start\""));
        let persisted_old_thread: i64 = crate::database::connect(&db_path)
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM source_sessions WHERE id='old-thread'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(persisted_old_thread, 0);
    }

    #[derive(Clone)]
    struct SharedBuffer(Arc<Mutex<Vec<u8>>>);

    impl Write for SharedBuffer {
        fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buffer);
            Ok(buffer.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn approval_requests_are_listed_and_responses_preserve_the_rpc_id() {
        let manager = AppServerManager::default();
        let output = Arc::new(Mutex::new(Vec::new()));
        manager
            .register_connection(
                42,
                Arc::new(Mutex::new(Box::new(SharedBuffer(Arc::clone(&output))))),
            )
            .unwrap();
        manager.handle_server_message(
            42,
            "cont-1",
            "project-1",
            &json!({
                "id": "approval-rpc-1",
                "method": "item/commandExecution/requestApproval",
                "params": {
                    "threadId": "thread-1",
                    "turnId": "turn-1",
                    "itemId": "item-1",
                    "startedAtMs": 123,
                    "reason": "需要访问网络",
                    "command": "curl https://example.com",
                    "cwd": "C:\\repo",
                    "networkApprovalContext": {"host": "example.com", "protocol": "https"}
                }
            }),
            None,
        );
        let requests = manager.list_requests().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].kind, "network");
        assert_eq!(requests[0].network_host.as_deref(), Some("example.com"));

        manager
            .respond(&requests[0].id, json!({"decision": "accept"}))
            .unwrap();
        assert!(manager.list_requests().unwrap().is_empty());
        let response = String::from_utf8(output.lock().unwrap().clone()).unwrap();
        assert!(response.contains("\"id\":\"approval-rpc-1\""));
        assert!(response.contains("\"decision\":\"accept\""));
    }

    #[test]
    fn permission_requests_only_grant_the_requested_profile() {
        let manager = AppServerManager::default();
        let output = Arc::new(Mutex::new(Vec::new()));
        manager
            .register_connection(
                43,
                Arc::new(Mutex::new(Box::new(SharedBuffer(Arc::clone(&output))))),
            )
            .unwrap();
        manager.handle_server_message(
            43,
            "cont-permission",
            "project-1",
            &json!({
                "id": 61,
                "method": "item/permissions/requestApproval",
                "params": {
                    "threadId": "thread-1",
                    "turnId": "turn-1",
                    "itemId": "item-2",
                    "cwd": "C:\\repo",
                    "permissions": {"network": {"enabled": true}},
                    "reason": "需要下载依赖",
                    "startedAtMs": 124
                }
            }),
            None,
        );
        let requests = manager.list_requests().unwrap();
        assert_eq!(requests[0].kind, "permissions");
        assert_eq!(
            requests[0].permissions,
            Some(json!({"network": {"enabled": true}}))
        );

        let error = manager
            .respond(
                &requests[0].id,
                json!({"permissions": {"network": {"enabled": false}}, "scope": "turn"}),
            )
            .unwrap_err();
        assert!(error.to_string().contains("未被请求的权限"));
        manager
            .respond(
                &requests[0].id,
                json!({"permissions": {"network": {"enabled": true}}, "scope": "session"}),
            )
            .unwrap();
        let response = String::from_utf8(output.lock().unwrap().clone()).unwrap();
        assert!(response.contains("\"id\":61"));
        assert!(response.contains("\"scope\":\"session\""));
    }

    #[test]
    fn elicitation_and_tool_input_use_their_protocol_specific_responses() {
        let manager = AppServerManager::default();
        let output = Arc::new(Mutex::new(Vec::new()));
        manager
            .register_connection(
                44,
                Arc::new(Mutex::new(Box::new(SharedBuffer(Arc::clone(&output))))),
            )
            .unwrap();
        manager.handle_server_message(
            44,
            "cont-interactive",
            "project-1",
            &json!({
                "id": "mcp-1",
                "method": "mcpServer/elicitation/request",
                "params": {
                    "threadId": "thread-1",
                    "turnId": "turn-1",
                    "serverName": "calendar",
                    "mode": "form",
                    "message": "选择日期",
                    "requestedSchema": {
                        "type": "object",
                        "properties": {"date": {"type": "string"}},
                        "required": ["date"]
                    }
                }
            }),
            None,
        );
        let elicitation = manager.list_requests().unwrap().remove(0);
        assert_eq!(elicitation.kind, "mcp_elicitation");
        manager
            .respond(
                &elicitation.id,
                json!({"action": "accept", "content": {"date": "2026-08-03"}}),
            )
            .unwrap();

        manager.handle_server_message(
            44,
            "cont-interactive",
            "project-1",
            &json!({
                "id": "input-1",
                "method": "item/tool/requestUserInput",
                "params": {
                    "threadId": "thread-1",
                    "turnId": "turn-1",
                    "itemId": "item-3",
                    "questions": [{
                        "id": "strategy",
                        "header": "策略",
                        "question": "如何继续？",
                        "options": [{"label": "安全", "description": "保守执行"}]
                    }]
                }
            }),
            None,
        );
        let input = manager.list_requests().unwrap().remove(0);
        assert_eq!(input.kind, "tool_user_input");
        manager
            .respond(
                &input.id,
                json!({"answers": {"strategy": {"answers": ["安全"]}}}),
            )
            .unwrap();
        let response = String::from_utf8(output.lock().unwrap().clone()).unwrap();
        assert!(response.contains("\"id\":\"mcp-1\""));
        assert!(response.contains("\"date\":\"2026-08-03\""));
        assert!(response.contains("\"id\":\"input-1\""));
        assert!(response.contains("\"strategy\":{\"answers\":[\"安全\"]}"));
    }

    #[test]
    fn duplicate_requests_are_deduplicated_and_resolution_notifications_clear_them() {
        let manager = AppServerManager::default();
        let output = Arc::new(Mutex::new(Vec::new()));
        manager
            .register_connection(
                45,
                Arc::new(Mutex::new(Box::new(SharedBuffer(Arc::clone(&output))))),
            )
            .unwrap();
        let request = json!({
            "id": "input-duplicate",
            "method": "item/tool/requestUserInput",
            "params": {
                "threadId": "thread-1",
                "turnId": "turn-1",
                "itemId": "item-4",
                "questions": [{"id": "name", "header": "名称", "question": "请输入名称"}]
            }
        });
        manager.handle_server_message(45, "cont-1", "project-1", &request, None);
        manager.handle_server_message(45, "cont-1", "project-1", &request, None);
        assert_eq!(manager.list_requests().unwrap().len(), 1);

        manager.handle_server_message(
            45,
            "cont-1",
            "project-1",
            &json!({
                "method": "serverRequest/resolved",
                "params": {"threadId": "thread-1", "requestId": "input-duplicate"}
            }),
            None,
        );
        assert!(manager.list_requests().unwrap().is_empty());
    }

    #[test]
    fn unsupported_server_requests_receive_an_error_instead_of_hanging() {
        let manager = AppServerManager::default();
        let output = Arc::new(Mutex::new(Vec::new()));
        manager
            .register_connection(
                7,
                Arc::new(Mutex::new(Box::new(SharedBuffer(Arc::clone(&output))))),
            )
            .unwrap();
        manager.handle_server_message(
            7,
            "cont-2",
            "project-2",
            &json!({"id": 99, "method": "item/tool/call", "params": {}}),
            None,
        );
        let response = String::from_utf8(output.lock().unwrap().clone()).unwrap();
        assert!(response.contains("\"id\":99"));
        assert!(response.contains("\"code\":-32601"));
    }
}
