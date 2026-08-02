use crate::{
    codex_runtime,
    error::{AppError, AppResult},
    models::CodexProfile,
};
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
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

type ThreadStartResult = Result<String, (AppError, Option<String>)>;

fn send(stdin: &mut impl Write, message: Value) -> AppResult<()> {
    serde_json::to_writer(&mut *stdin, &message)?;
    stdin.write_all(b"\n")?;
    stdin.flush()?;
    Ok(())
}

fn start_thread(
    stdin: &mut impl Write,
    receiver: &Receiver<Value>,
    profile: Option<&CodexProfile>,
    working_directory: &str,
    target_model: &str,
    prompt: &str,
) -> ThreadStartResult {
    send(
        stdin,
        json!({"method":"initialize","id":1,"params":{"clientInfo":{"name":"continuum","title":"Continuum","version":env!("CARGO_PKG_VERSION")}}}),
    )
    .map_err(|error| (error, None))?;
    response(receiver, 1).map_err(|error| (error, None))?;
    send(stdin, json!({"method":"initialized","params":{}})).map_err(|error| (error, None))?;
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
    send(
        stdin,
        json!({"method":"thread/start","id":2,"params":start_params}),
    )
    .map_err(|error| (error, None))?;
    let started = response(receiver, 2).map_err(|error| (error, None))?;
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
    send(
        stdin,
        json!({"method":"turn/start","id":3,"params":{"threadId":thread_id,"cwd":working_directory,"input":[{"type":"text","text":prompt}]}}),
    )
    .map_err(|error| (error, Some(thread_id.clone())))?;
    response(receiver, 3).map_err(|error| (error, Some(thread_id.clone())))?;
    Ok(thread_id)
}

fn response(receiver: &Receiver<Value>, id: i64) -> AppResult<Value> {
    let deadline = Instant::now() + RESPONSE_TIMEOUT;
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

fn terminate(mut child: Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn keep_alive(mut child: Child, stdin: ChildStdin) {
    thread::spawn(move || {
        let _stdin = stdin;
        let _ = child.wait();
    });
}

fn start_reader(stdout: impl std::io::Read + Send + 'static) -> Receiver<Value> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Ok(message) = serde_json::from_str::<Value>(&line) {
                let _ = sender.send(message);
            }
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
    command: &str,
    profile: Option<&CodexProfile>,
    working_directory: &str,
    target_model: &str,
    prompt: &str,
) -> Result<AppServerLaunch, AppServerLaunchFailure> {
    if profile.is_some_and(|value| !value.launch_arguments.is_empty()) {
        return Err(AppServerLaunchFailure {
            process_id: 0,
            thread_id: None,
            message: "Codex Profile 含 CLI 专用启动参数，不能通过 App Server 无损启动".into(),
        });
    }
    let mut process = codex_runtime::command(command).map_err(|error| AppServerLaunchFailure {
        process_id: 0,
        thread_id: None,
        message: error.to_string(),
    })?;
    configure(&mut process, working_directory);
    let mut child = process.spawn().map_err(|error| AppServerLaunchFailure {
        process_id: 0,
        thread_id: None,
        message: format!("无法启动 Codex App Server：{error}"),
    })?;
    let process_id = child.id();
    let Some(mut stdin) = child.stdin.take() else {
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
    let receiver = start_reader(stdout);
    let result = start_thread(
        &mut stdin,
        &receiver,
        profile,
        working_directory,
        target_model,
        prompt,
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
            keep_alive(child, stdin);
            Ok(launch)
        }
        Err(error) => {
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
    let receiver = start_reader(stdout);
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
    use std::io::Cursor;

    #[test]
    fn response_skips_notifications_and_returns_matching_id() {
        let (sender, receiver) = mpsc::channel();
        sender.send(json!({"method":"thread/started"})).unwrap();
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
        let thread_id = start_thread(
            &mut output,
            &receiver,
            None,
            "C:\\repo",
            "default",
            "CONTINUATION_ID=abc",
        )
        .unwrap();
        assert_eq!(thread_id, "thread-2");
        let lines = String::from_utf8(output.into_inner()).unwrap();
        assert!(lines.contains("\"method\":\"initialize\""));
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
        let (_, thread_id) = start_thread(
            &mut output,
            &receiver,
            None,
            "C:\\repo",
            "default",
            "CONTINUATION_ID=partial",
        )
        .unwrap_err();
        assert_eq!(thread_id.as_deref(), Some("partial-thread"));
    }
}
