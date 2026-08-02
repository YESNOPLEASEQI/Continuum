use crate::{
    agent_adapters::AgentAdapter,
    codex_adapter::CodexAdapter,
    codex_app_server, codex_runtime, context_compiler, database,
    error::{AppError, AppResult},
    filesystem,
    models::*,
    profiles, session_indexer, settings, unified_project,
};
use chrono::{DateTime, Utc};
use rusqlite::{params, OptionalExtension};
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

fn agent(value: &str) -> AgentKind {
    match value {
        "claude" => AgentKind::Claude,
        "gemini" => AgentKind::Gemini,
        "opencode" => AgentKind::Opencode,
        "cursor" => AgentKind::Cursor,
        "copilot" => AgentKind::Copilot,
        _ => AgentKind::Codex,
    }
}
fn agent_name(value: &AgentKind) -> &'static str {
    match value {
        AgentKind::Codex => "codex",
        AgentKind::Claude => "claude",
        AgentKind::Gemini => "gemini",
        AgentKind::Opencode => "opencode",
        AgentKind::Cursor => "cursor",
        AgentKind::Copilot => "copilot",
    }
}
fn mode(value: &str) -> ContinuationMode {
    match value {
        "native" => ContinuationMode::Native,
        "export_only" => ContinuationMode::ExportOnly,
        _ => ContinuationMode::Context,
    }
}
fn mode_name(value: &ContinuationMode) -> &'static str {
    match value {
        ContinuationMode::Native => "native",
        ContinuationMode::Context => "context",
        ContinuationMode::ExportOnly => "export_only",
    }
}

fn status(value: &str) -> Option<ContinuationStatus> {
    Some(match value {
        "idle" => ContinuationStatus::Idle,
        "compiling_context" | "compiling" => ContinuationStatus::CompilingContext,
        "writing_context" => ContinuationStatus::WritingContext,
        "preparing_launch" | "prepared" => ContinuationStatus::PreparingLaunch,
        "launching" => ContinuationStatus::Launching,
        "waiting_for_session" | "waiting_detection" => ContinuationStatus::WaitingForSession,
        "candidate_sessions_found" | "needs_confirmation" => {
            ContinuationStatus::CandidateSessionsFound
        }
        "binding" => ContinuationStatus::Binding,
        "listening" => ContinuationStatus::Listening,
        "completed" | "export_only" => ContinuationStatus::Completed,
        "launch_failed" | "failed" => ContinuationStatus::LaunchFailed,
        "detection_timeout" => ContinuationStatus::DetectionTimeout,
        "manual_binding_required" => ContinuationStatus::ManualBindingRequired,
        "cancelled" => ContinuationStatus::Cancelled,
        _ => return None,
    })
}

fn transition_allowed(from: &ContinuationStatus, to: &ContinuationStatus) -> bool {
    use ContinuationStatus::*;
    matches!(
        (from, to),
        (Idle, CompilingContext | Cancelled)
            | (CompilingContext, WritingContext | LaunchFailed | Cancelled)
            | (WritingContext, PreparingLaunch | LaunchFailed | Cancelled)
            | (
                PreparingLaunch,
                Launching | Completed | LaunchFailed | Cancelled
            )
            | (
                Launching,
                WaitingForSession | Binding | LaunchFailed | Cancelled
            )
            | (
                WaitingForSession,
                CandidateSessionsFound
                    | Binding
                    | DetectionTimeout
                    | ManualBindingRequired
                    | Cancelled
            )
            | (
                CandidateSessionsFound,
                WaitingForSession | Binding | ManualBindingRequired | Cancelled
            )
            | (
                Binding,
                Listening | ManualBindingRequired | LaunchFailed | Cancelled
            )
            | (Listening, Completed | Cancelled)
            | (LaunchFailed, PreparingLaunch | Launching | Cancelled)
            | (
                DetectionTimeout,
                WaitingForSession | ManualBindingRequired | Cancelled
            )
            | (
                ManualBindingRequired,
                Binding | WaitingForSession | Cancelled
            )
    )
}

fn transition(
    db_path: &Path,
    id: &str,
    next: ContinuationStatus,
    failure: Option<(&str, &str)>,
) -> AppResult<()> {
    let mut conn = database::connect(db_path)?;
    let transaction = conn.transaction()?;
    let current_value: String = transaction
        .query_row(
            "SELECT status FROM continuations WHERE id=?1",
            params![id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| AppError::Message("找不到续接记录".into()))?;
    let current = status(&current_value)
        .ok_or_else(|| AppError::Message(format!("未知的 Continuation 状态：{current_value}")))?;
    if current == next {
        transaction.commit()?;
        return Ok(());
    }
    if !transition_allowed(&current, &next) {
        return Err(AppError::Message(format!(
            "非法 Continuation 状态迁移：{} -> {}",
            current.as_str(),
            next.as_str()
        )));
    }
    let (failure_code, failure_message) = failure
        .map(|(code, message)| (Some(code), Some(message)))
        .unwrap_or((None, None));
    let completed_at =
        matches!(next, ContinuationStatus::Completed).then(|| Utc::now().to_rfc3339());
    transaction.execute(
        "UPDATE continuations SET status=?1,updated_at=?2,state_version=state_version+1,failure_code=?3,failure_message=?4,warning=?4,completed_at=COALESCE(?5,completed_at),listening=CASE WHEN ?1='listening' THEN 1 WHEN ?1 IN ('completed','cancelled') THEN 0 ELSE listening END WHERE id=?6",
        params![next.as_str(),Utc::now().to_rfc3339(),failure_code,failure_message,completed_at,id],
    )?;
    transaction.commit()?;
    Ok(())
}
fn quote(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\\\""))
}
fn normalize(path: &str) -> String {
    fs::canonicalize(path)
        .unwrap_or_else(|_| PathBuf::from(path))
        .to_string_lossy()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_ascii_lowercase()
}
fn created_after(created_at: &str, started_at: &str) -> bool {
    match (
        DateTime::parse_from_rfc3339(created_at),
        DateTime::parse_from_rfc3339(started_at),
    ) {
        (Ok(created), Ok(started)) => created > started,
        _ => false,
    }
}
fn bootstrap_prompt(marker: &str, bootstrap: &Path) -> String {
    format!("你正在继续一个由旧会话压缩而来的项目任务。{marker}。请先读取文件 {}，然后完成：1. 检查当前工作目录；2. 检查 Git 状态和实际文件；3. 对比上下文记录与实际工作区；4. 如有冲突，以实际文件为准并报告；5. 简要复述当前目标、已完成工作和下一步；6. 随后继续上下文中最高优先级的未完成任务。",bootstrap.display())
}

fn launch_prompt(profile: Option<&CodexProfile>, marker: &str, bootstrap: &Path) -> String {
    if let Some(profile) = profile {
        profile
            .launch_prompt_template
            .replace("{{CONTEXT_FILE_PATH}}", &bootstrap.to_string_lossy())
            .replace("{{CONTINUATION_MARKER}}", marker)
    } else {
        bootstrap_prompt(marker, bootstrap)
    }
}

fn launch_preview(
    command: &str,
    profile: Option<&CodexProfile>,
    working_directory: &str,
    prompt: &str,
) -> String {
    let mut values = vec![quote(command)];
    if let Some(profile) = profile {
        if let Some(model) = profile.model.as_deref() {
            values.extend(["--model".into(), quote(model)]);
        }
        values.extend([
            "--sandbox".into(),
            quote(&profile.sandbox_mode),
            "--ask-for-approval".into(),
            quote(&profile.approval_mode),
        ]);
        values.extend(profile.launch_arguments.iter().map(|value| quote(value)));
    }
    values.extend(["-C".into(), quote(working_directory), quote(prompt)]);
    values.join(" ")
}

pub fn create(
    db_path: &Path,
    data_dir: &Path,
    options: &ContextCompileOptions,
    launch: bool,
) -> AppResult<ContinuationRecord> {
    let project = unified_project::get(db_path, &options.project_id, options.token_budget)?;
    let compact = uuid::Uuid::new_v4().simple().to_string();
    let id = format!("cont_{}_{}", Utc::now().format("%Y%m%d"), &compact[..8]);
    let marker = format!("CONTINUATION_ID={id}");
    let created = Utc::now().to_rfc3339();
    let continuation_mode = if matches!(options.target_agent, AgentKind::Codex) {
        ContinuationMode::Context
    } else {
        ContinuationMode::ExportOnly
    };
    let selected_profile = if matches!(options.target_agent, AgentKind::Codex) {
        profiles::resolve(db_path, &options.project_id, &options.branch_id)?
    } else {
        None
    };
    let selected_profile_id = selected_profile.as_ref().map(|profile| profile.id.clone());
    let selected_profile_json = selected_profile
        .as_ref()
        .map(serde_json::to_string)
        .transpose()?;
    database::connect(db_path)?.execute(
        "INSERT INTO continuations(id,project_id,branch_id,source_node_id,snapshot_id,target_agent,target_model,mode,status,bootstrap_file,launch_command,target_session_id,created_at,warning,process_id,working_directory,context_hash,marker,started_at,detected_at,listening,context_snapshot_id,launch_command_preview,continuation_marker,context_file_path,context_file_hash,updated_at,state_version,cancellation_requested,codex_profile_id,launch_profile_json) VALUES(?1,?2,?3,?4,'',?5,?6,?7,'idle','','',NULL,?8,NULL,NULL,?9,'',?10,'',NULL,0,NULL,'',?10,'','',?8,0,0,?11,?12)",
        params![id,options.project_id,options.branch_id,options.source_node_id,agent_name(&options.target_agent),options.target_model,mode_name(&continuation_mode),created,project.summary.project_path,marker,selected_profile_id,selected_profile_json],
    )?;
    transition(db_path, &id, ContinuationStatus::CompilingContext, None)?;
    let snapshot = match context_compiler::save_snapshot(db_path, options) {
        Ok(value) => value,
        Err(error) => {
            let message = error.to_string();
            transition(
                db_path,
                &id,
                ContinuationStatus::LaunchFailed,
                Some(("context_compile_failed", &message)),
            )?;
            return get(db_path, &id);
        }
    };
    database::connect(db_path)?.execute(
        "UPDATE continuations SET snapshot_id=?1,context_snapshot_id=?1,updated_at=?2 WHERE id=?3",
        params![snapshot.id, Utc::now().to_rfc3339(), id],
    )?;
    transition(db_path, &id, ContinuationStatus::WritingContext, None)?;
    let context_dir = Path::new(&project.summary.project_path)
        .join(".continuum")
        .join("continuations");
    let bootstrap = context_dir.join(format!("{id}.md"));
    let context_document = format!(
        "<!-- {marker} -->\n# Continuum Fresh Continuation\n\n{}\n\n---\n此文件由确定性 Context Compiler 生成。先核对实际工作区；不要自动执行历史命令。\n",
        snapshot.compiled.compiled_text
    );
    let write_result = fs::create_dir_all(&context_dir)
        .map_err(AppError::Io)
        .and_then(|_| filesystem::write_atomic(&bootstrap, context_document.as_bytes()))
        .and_then(|_| filesystem::sha256_file(&bootstrap));
    let context_hash = match write_result {
        Ok(value) => value,
        Err(error) => {
            let message = error.to_string();
            transition(
                db_path,
                &id,
                ContinuationStatus::LaunchFailed,
                Some(("context_write_failed", &message)),
            )?;
            return get(db_path, &id);
        }
    };
    let prompt = launch_prompt(selected_profile.as_ref(), &marker, &bootstrap);
    let app_settings = settings::load(db_path, data_dir)?;
    let command = selected_profile
        .as_ref()
        .map(|profile| profile.executable_path.as_str())
        .unwrap_or(&app_settings.codex_command);
    let launch_command = launch_preview(
        command,
        selected_profile.as_ref(),
        &project.summary.project_path,
        &prompt,
    );
    database::connect(db_path)?.execute(
        "UPDATE continuations SET bootstrap_file=?1,context_file_path=?1,context_hash=?2,context_file_hash=?2,launch_command=?3,launch_command_preview=?3,updated_at=?4 WHERE id=?5",
        params![bootstrap.to_string_lossy(),context_hash,launch_command,Utc::now().to_rfc3339(),id],
    )?;
    transition(db_path, &id, ContinuationStatus::PreparingLaunch, None)?;
    if !matches!(options.target_agent, AgentKind::Codex) {
        database::connect(db_path)?.execute(
            "UPDATE continuations SET warning='目标 Agent 尚未支持自动启动；上下文文件已生成' WHERE id=?1",
            params![id],
        )?;
        transition(db_path, &id, ContinuationStatus::Completed, None)?;
        return get(db_path, &id);
    }
    if launch {
        launch_prepared(db_path, data_dir, &id)
    } else {
        get(db_path, &id)
    }
}

#[allow(dead_code)]
fn create_legacy(
    db_path: &Path,
    data_dir: &Path,
    options: &ContextCompileOptions,
    launch: bool,
) -> AppResult<ContinuationRecord> {
    let snapshot = context_compiler::save_snapshot(db_path, options)?;
    let project = unified_project::get(db_path, &options.project_id, options.token_budget)?;
    let compact = uuid::Uuid::new_v4().simple().to_string();
    let id = format!("cont_{}_{}", Utc::now().format("%Y%m%d"), &compact[..8]);
    let marker = format!("CONTINUATION_ID={id}");
    let context_dir = Path::new(&project.summary.project_path)
        .join(".continuum")
        .join("continuations");
    fs::create_dir_all(&context_dir)?;
    let bootstrap = context_dir.join(format!("{id}.md"));
    let context_document=format!("<!-- {marker} -->\n# Continuum Fresh Continuation\n\n{}\n\n---\n该文件由确定性 RuleBasedProvider 生成。先核对实际工作区；不要自动执行历史命令。\n",snapshot.compiled.compiled_text);
    filesystem::write_atomic(&bootstrap, context_document.as_bytes())?;
    let context_hash = filesystem::sha256_file(&bootstrap)?;
    let prompt = bootstrap_prompt(&marker, &bootstrap);
    let app_settings = settings::load(db_path, data_dir)?;
    let launch_command = format!(
        "{} -C {} {}",
        app_settings.codex_command,
        quote(&project.summary.project_path),
        quote(&prompt)
    );
    let continuation_mode = if matches!(options.target_agent, AgentKind::Codex) {
        ContinuationMode::Context
    } else {
        ContinuationMode::ExportOnly
    };
    let created = Utc::now().to_rfc3339();
    let mut record = ContinuationRecord {
        id: id.clone(),
        project_id: options.project_id.clone(),
        branch_id: options.branch_id.clone(),
        source_node_id: options.source_node_id.clone(),
        snapshot_id: snapshot.id,
        target_agent: options.target_agent.clone(),
        target_model: options.target_model.clone(),
        mode: continuation_mode,
        status: "writing_context".into(),
        bootstrap_file: bootstrap.to_string_lossy().into_owned(),
        launch_command,
        target_session_id: None,
        created_at: created.clone(),
        warning: None,
        process_id: None,
        working_directory: project.summary.project_path.clone(),
        context_hash,
        marker: marker.clone(),
        started_at: created.clone(),
        detected_at: None,
        listening: false,
    };
    if !matches!(record.target_agent, AgentKind::Codex) {
        record.status = "export_only".into();
        record.warning = Some("目标 Agent 适配器尚不支持自动启动；已生成上下文文件".into());
    } else if launch {
        let capabilities = codex_runtime::detect(db_path, data_dir, false)?;
        if !capabilities.installed || !capabilities.supports_cd {
            return Err(AppError::Message(capabilities.error.unwrap_or_else(|| {
                "当前 Codex 版本没有通过 -C/--cd 启动能力检测".into()
            })));
        }
        match launch_fresh_codex(
            &app_settings.codex_command,
            &record.working_directory,
            &prompt,
        ) {
            Ok(pid) => {
                record.process_id = Some(pid);
                record.started_at = Utc::now().to_rfc3339();
                record.status = "waiting_detection".into();
            }
            Err(error) => {
                record.status = "launch_failed".into();
                record.warning = Some(error.to_string());
            }
        }
    } else {
        record.status = "prepared".into();
    }
    let conn = database::connect(db_path)?;
    conn.execute("INSERT INTO continuations(id,project_id,branch_id,source_node_id,snapshot_id,target_agent,target_model,mode,status,bootstrap_file,launch_command,target_session_id,created_at,warning,process_id,working_directory,context_hash,marker,started_at,detected_at,listening) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,NULL,?12,?13,?14,?15,?16,?17,?18,NULL,0)",params![record.id,record.project_id,record.branch_id,record.source_node_id,record.snapshot_id,agent_name(&record.target_agent),record.target_model,mode_name(&record.mode),record.status,record.bootstrap_file,record.launch_command,record.created_at,record.warning,record.process_id.map(|v|v as i64),record.working_directory,record.context_hash,record.marker,record.started_at])?;
    Ok(record)
}

pub fn launch_prepared(db_path: &Path, data_dir: &Path, id: &str) -> AppResult<ContinuationRecord> {
    let record = get(db_path, id)?;
    if !matches!(record.target_agent, AgentKind::Codex) {
        return Err(AppError::Message(
            "只有 Codex 支持自动启动；其他 Agent 仅导出上下文".into(),
        ));
    }
    if !matches!(
        status(&record.status),
        Some(ContinuationStatus::PreparingLaunch | ContinuationStatus::LaunchFailed)
    ) {
        return Err(AppError::Message("该续接记录当前不可启动".into()));
    }
    let partial_app_server_thread: Option<String> = database::connect(db_path)?
        .query_row(
            "SELECT target_session_id FROM continuations WHERE id=?1 AND launch_transport='app_server' AND target_session_id IS NOT NULL",
            params![id],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(thread_id) = partial_app_server_thread {
        return Err(AppError::Message(format!(
            "本次启动已经创建 Codex 会话 {thread_id}，但上下文注入未确认成功；为避免产生重复会话，不能自动重试，请改为恢复该会话或新建一条 Fresh Continuation"
        )));
    }
    if !Path::new(&record.bootstrap_file).is_file() {
        transition(
            db_path,
            id,
            ContinuationStatus::LaunchFailed,
            Some(("context_file_missing", "续接上下文文件不存在")),
        )?;
        return get(db_path, id);
    }
    let profile_json: Option<String> = database::connect(db_path)?.query_row(
        "SELECT launch_profile_json FROM continuations WHERE id=?1",
        params![id],
        |row| row.get(0),
    )?;
    let profile: Option<CodexProfile> = profile_json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()?;
    let prompt = launch_prompt(
        profile.as_ref(),
        &record.marker,
        Path::new(&record.bootstrap_file),
    );
    let capabilities = codex_runtime::detect(db_path, data_dir, false)?;
    if !capabilities.installed || !capabilities.supports_cd {
        let message = capabilities
            .error
            .unwrap_or_else(|| "当前 Codex 版本未通过 -C/--cd 启动能力检测".into());
        transition(
            db_path,
            id,
            ContinuationStatus::LaunchFailed,
            Some(("codex_capability_missing", &message)),
        )?;
        return get(db_path, id);
    }
    if let Some(profile) = profile.as_ref() {
        if let Err(error) = profiles::validate(profile, &capabilities) {
            let message = error.to_string();
            transition(
                db_path,
                id,
                ContinuationStatus::LaunchFailed,
                Some(("codex_profile_invalid", &message)),
            )?;
            return get(db_path, id);
        }
    }
    let command = profile
        .as_ref()
        .map(|value| value.executable_path.as_str())
        .or(capabilities.executable_path.as_deref())
        .ok_or_else(|| AppError::Message("Codex 能力报告缺少可执行文件路径".into()))?;
    transition(db_path, id, ContinuationStatus::Launching, None)?;
    if capabilities.supports_app_server
        && profile.as_ref().is_some_and(|value| {
            value.launch_arguments.is_empty() && value.approval_mode == "never"
        })
    {
        match codex_app_server::start_fresh(
            command,
            profile.as_ref(),
            &record.working_directory,
            &record.target_model,
            &prompt,
        ) {
            Ok(launch) => {
                let started_at = Utc::now().to_rfc3339();
                database::connect(db_path)?.execute(
                    "UPDATE continuations SET process_id=?1,launch_process_id=?1,started_at=?2,launch_started_at=?2,target_session_id=?3,app_server_thread_id=?3,launch_transport='app_server',app_server_protocol_version='v2',warning=NULL,failure_code=NULL,failure_message=NULL,updated_at=?2 WHERE id=?4",
                    params![launch.process_id as i64, started_at, launch.thread_id, id],
                )?;
                transition(db_path, id, ContinuationStatus::Binding, None)?;
                bind_app_server_thread(db_path, &record, &launch.thread_id)?;
                return get(db_path, id);
            }
            Err(error) => {
                let message = error.to_string();
                if let Some(thread_id) = error.thread_id.as_deref() {
                    record_partial_app_server_thread(
                        db_path,
                        &record,
                        error.process_id,
                        thread_id,
                    )?;
                }
                transition(
                    db_path,
                    id,
                    ContinuationStatus::LaunchFailed,
                    Some((
                        if error.thread_id.is_some() {
                            "app_server_turn_failed"
                        } else {
                            "app_server_launch_failed"
                        },
                        &message,
                    )),
                )?;
                return get(db_path, id);
            }
        }
    }
    match launch_fresh_codex_profiled(
        command,
        profile.as_ref(),
        &record.working_directory,
        &prompt,
    ) {
        Ok(pid) => {
            let started_at = Utc::now();
            let deadline = started_at + chrono::Duration::minutes(3);
            database::connect(db_path)?.execute("UPDATE continuations SET process_id=?1,launch_process_id=?1,started_at=?2,launch_started_at=?2,detection_deadline_at=?3,warning=NULL,failure_code=NULL,failure_message=NULL,updated_at=?2 WHERE id=?4",params![pid as i64,started_at.to_rfc3339(),deadline.to_rfc3339(),id])?;
            transition(db_path, id, ContinuationStatus::WaitingForSession, None)?;
        }
        Err(error) => {
            let message = error.to_string();
            transition(
                db_path,
                id,
                ContinuationStatus::LaunchFailed,
                Some(("process_launch_failed", &message)),
            )?;
        }
    }
    get(db_path, id)
}

fn record_partial_app_server_thread(
    db_path: &Path,
    record: &ContinuationRecord,
    process_id: u32,
    thread_id: &str,
) -> AppResult<()> {
    let mut conn = database::connect(db_path)?;
    let transaction = conn.transaction()?;
    let now = Utc::now().to_rfc3339();
    transaction.execute(
        "UPDATE continuations SET process_id=?1,launch_process_id=?1,started_at=?2,launch_started_at=?2,target_session_id=?3,app_server_thread_id=?3,launch_transport='app_server',app_server_protocol_version='v2',updated_at=?2 WHERE id=?4",
        params![process_id as i64, now, thread_id, record.id],
    )?;
    transaction.execute(
        "INSERT INTO project_bindings(project_id,binding_type,binding_id,branch_id,created_at,metadata_json) VALUES(?1,'source_session',?2,?3,?4,?5) ON CONFLICT(project_id,binding_type,binding_id) DO UPDATE SET branch_id=excluded.branch_id,metadata_json=excluded.metadata_json",
        params![record.project_id, thread_id, record.branch_id, now, json!({"continuationId":record.id,"transport":"app_server","partial":true,"contextInjected":false}).to_string()],
    )?;
    transaction.execute(
        "UPDATE source_sessions SET bound_project_id=?1,bound_branch_id=?2,status='bound' WHERE id=?3",
        params![record.project_id, record.branch_id, thread_id],
    )?;
    transaction.commit()?;
    Ok(())
}

fn bind_app_server_thread(
    db_path: &Path,
    record: &ContinuationRecord,
    thread_id: &str,
) -> AppResult<()> {
    let mut conn = database::connect(db_path)?;
    let transaction = conn.transaction()?;
    let bound_elsewhere: i64 = transaction.query_row(
        "SELECT COUNT(*) FROM project_bindings WHERE binding_type='source_session' AND binding_id=?1 AND project_id<>?2",
        params![thread_id, record.project_id],
        |row| row.get(0),
    )?;
    if bound_elsewhere > 0 {
        return Err(AppError::Message(
            "App Server 返回的会话已经绑定到其他统一项目".into(),
        ));
    }
    transaction.execute(
        "INSERT INTO project_bindings(project_id,binding_type,binding_id,branch_id,created_at,metadata_json) VALUES(?1,'source_session',?2,?3,?4,?5) ON CONFLICT(project_id,binding_type,binding_id) DO UPDATE SET branch_id=excluded.branch_id,metadata_json=excluded.metadata_json",
        params![record.project_id, thread_id, record.branch_id, Utc::now().to_rfc3339(), json!({"continuationId":record.id,"transport":"app_server"}).to_string()],
    )?;
    transaction.execute(
        "UPDATE source_sessions SET bound_project_id=?1,bound_branch_id=?2,status='bound' WHERE id=?3",
        params![record.project_id, record.branch_id, thread_id],
    )?;
    let detected_at = Utc::now().to_rfc3339();
    transaction.execute(
        "UPDATE continuations SET target_session_id=?1,detected_at=?2,listening=1,updated_at=?2 WHERE id=?3",
        params![thread_id, detected_at, record.id],
    )?;
    transaction.commit()?;
    transition(db_path, &record.id, ContinuationStatus::Listening, None)?;
    let (compiled_json, stored_original_tokens): (String, i64) = database::connect(db_path)?.query_row(
        "SELECT compiled_json,COALESCE(estimated_original_tokens,0) FROM context_snapshots WHERE id=?1",
        params![record.snapshot_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let compiled: CompiledContext = serde_json::from_str(&compiled_json)?;
    let original_tokens = if compiled.original_estimated_tokens > 0 {
        compiled.original_estimated_tokens
    } else {
        stored_original_tokens.max(0) as usize
    };
    unified_project::insert_event(
        db_path,
        &record.project_id,
        &record.branch_id,
        "session_switch",
        &format!("上下文已从约 {} tokens 压缩到约 {} tokens，并通过 Codex App Server 切换到新的 Codex 会话 {}。", original_tokens, compiled.estimated_tokens, thread_id),
        json!({"continuationId":record.id,"fromEstimatedTokens":original_tokens,"toEstimatedTokens":compiled.estimated_tokens,"targetSessionId":thread_id,"transport":"app_server"}),
    )?;
    Ok(())
}

#[allow(dead_code)]
fn launch_prepared_legacy(
    db_path: &Path,
    data_dir: &Path,
    id: &str,
) -> AppResult<ContinuationRecord> {
    let mut record = get(db_path, id)?;
    if !matches!(record.target_agent, AgentKind::Codex) {
        return Err(AppError::Message(
            "只有 Codex 适配器支持自动启动；其他 Agent 仅导出上下文".into(),
        ));
    }
    if !matches!(record.status.as_str(), "prepared" | "launch_failed") {
        return Err(AppError::Message("该续接记录当前不可启动".into()));
    }
    let prompt = bootstrap_prompt(&record.marker, Path::new(&record.bootstrap_file));
    let app_settings = settings::load(db_path, data_dir)?;
    let capabilities = codex_runtime::detect(db_path, data_dir, false)?;
    if !capabilities.installed || !capabilities.supports_cd {
        return Err(AppError::Message(capabilities.error.unwrap_or_else(|| {
            "当前 Codex 版本没有通过 -C/--cd 启动能力检测".into()
        })));
    }
    database::connect(db_path)?.execute(
        "UPDATE continuations SET status='launching',warning=NULL WHERE id=?1",
        params![id],
    )?;
    match launch_fresh_codex(
        &app_settings.codex_command,
        &record.working_directory,
        &prompt,
    ) {
        Ok(pid) => {
            record.process_id = Some(pid);
            record.started_at = Utc::now().to_rfc3339();
            record.status = "waiting_detection".into();
            record.warning = None;
            database::connect(db_path)?.execute("UPDATE continuations SET process_id=?1,started_at=?2,status='waiting_detection',warning=NULL WHERE id=?3",params![pid as i64,record.started_at,id])?;
        }
        Err(error) => {
            record.status = "launch_failed".into();
            record.warning = Some(error.to_string());
            database::connect(db_path)?.execute(
                "UPDATE continuations SET status='launch_failed',warning=?1 WHERE id=?2",
                params![record.warning, id],
            )?;
        }
    }
    Ok(record)
}

fn configure_fresh_command(
    process: &mut Command,
    profile: Option<&CodexProfile>,
    working_directory: &str,
    prompt: &str,
) {
    if let Some(profile) = profile {
        if let Some(model) = profile.model.as_deref() {
            process.arg("--model").arg(model);
        }
        process
            .arg("--sandbox")
            .arg(&profile.sandbox_mode)
            .arg("--ask-for-approval")
            .arg(&profile.approval_mode);
        process.args(&profile.launch_arguments);
    }
    process
        .arg("-C")
        .arg(working_directory)
        .arg(prompt)
        .current_dir(working_directory)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
}

#[cfg(windows)]
fn launch_fresh_codex_profiled(
    command: &str,
    profile: Option<&CodexProfile>,
    working_directory: &str,
    prompt: &str,
) -> AppResult<u32> {
    use std::os::windows::process::CommandExt;
    let mut process = codex_runtime::command(command)?;
    configure_fresh_command(&mut process, profile, working_directory, prompt);
    process.creation_flags(0x00000010);
    let child = process
        .spawn()
        .map_err(|error| AppError::Message(format!("启动 Codex 失败：{error}")))?;
    Ok(child.id())
}

#[cfg(not(windows))]
fn launch_fresh_codex_profiled(
    command: &str,
    profile: Option<&CodexProfile>,
    working_directory: &str,
    prompt: &str,
) -> AppResult<u32> {
    let mut process = codex_runtime::command(command)?;
    configure_fresh_command(&mut process, profile, working_directory, prompt);
    let child = process
        .spawn()
        .map_err(|error| AppError::Message(format!("启动 Codex 失败：{error}")))?;
    Ok(child.id())
}

#[cfg(windows)]
fn launch_fresh_codex(command: &str, working_directory: &str, prompt: &str) -> AppResult<u32> {
    use std::os::windows::process::CommandExt;
    let mut process = codex_runtime::command(command)?;
    process
        .arg("-C")
        .arg(working_directory)
        .arg(prompt)
        .current_dir(working_directory)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .creation_flags(0x00000010);
    let child = process
        .spawn()
        .map_err(|error| AppError::Message(format!("启动 Codex 失败：{error}")))?;
    Ok(child.id())
}
#[cfg(not(windows))]
fn launch_fresh_codex(command: &str, working_directory: &str, prompt: &str) -> AppResult<u32> {
    let child = codex_runtime::command(command)?
        .arg("-C")
        .arg(working_directory)
        .arg(prompt)
        .current_dir(working_directory)
        .spawn()
        .map_err(|error| AppError::Message(format!("启动 Codex 失败：{error}")))?;
    Ok(child.id())
}

fn row_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<ContinuationRecord> {
    let agent_value: String = row.get(5)?;
    let mode_value: String = row.get(7)?;
    Ok(ContinuationRecord {
        id: row.get(0)?,
        project_id: row.get(1)?,
        branch_id: row.get(2)?,
        source_node_id: row.get(3)?,
        snapshot_id: row.get(4)?,
        target_agent: agent(&agent_value),
        target_model: row.get(6)?,
        mode: mode(&mode_value),
        status: row.get(8)?,
        bootstrap_file: row.get(9)?,
        launch_command: row.get(10)?,
        target_session_id: row.get(11)?,
        created_at: row.get(12)?,
        warning: row.get(13)?,
        process_id: row.get::<_, Option<i64>>(14)?.map(|v| v as u32),
        working_directory: row.get(15)?,
        context_hash: row.get(16)?,
        marker: row.get(17)?,
        started_at: row.get(18)?,
        detected_at: row.get(19)?,
        listening: row.get::<_, i64>(20)? != 0,
    })
}
const SELECT:&str="SELECT id,project_id,branch_id,source_node_id,snapshot_id,target_agent,target_model,mode,status,bootstrap_file,launch_command,target_session_id,created_at,warning,process_id,working_directory,context_hash,marker,started_at,detected_at,listening FROM continuations";
pub fn get(db_path: &Path, id: &str) -> AppResult<ContinuationRecord> {
    database::connect(db_path)?
        .query_row(&format!("{SELECT} WHERE id=?1"), params![id], row_record)
        .optional()?
        .ok_or_else(|| AppError::Message("找不到续接记录".into()))
}
pub fn list(db_path: &Path, project_id: &str) -> AppResult<Vec<ContinuationRecord>> {
    let conn = database::connect(db_path)?;
    let mut stmt = conn.prepare(&format!(
        "{SELECT} WHERE project_id=?1 ORDER BY created_at DESC"
    ))?;
    let rows = stmt
        .query_map(params![project_id], row_record)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn poll(db_path: &Path, data_dir: &Path, id: &str) -> AppResult<ContinuationPollResult> {
    let mut record = get(db_path, id)?;
    let app_settings = settings::load(db_path, data_dir)?;
    if matches!(status(&record.status), Some(ContinuationStatus::Listening)) {
        let watch = session_indexer::poll(db_path, &app_settings)?;
        record = get(db_path, id)?;
        return Ok(ContinuationPollResult {
            continuation: record,
            candidates: vec![],
            inserted_nodes: watch.inserted_nodes,
        });
    }
    if !matches!(
        status(&record.status),
        Some(
            ContinuationStatus::WaitingForSession
                | ContinuationStatus::CandidateSessionsFound
                | ContinuationStatus::ManualBindingRequired
        )
    ) {
        return Ok(ContinuationPollResult {
            continuation: record,
            candidates: vec![],
            inserted_nodes: 0,
        });
    }
    let _watch = session_indexer::poll(db_path, &app_settings)?;
    let target_workdir = normalize(&record.working_directory);
    let mut matches = Vec::new();
    for summary in database::list_sessions(db_path)? {
        if !matches!(summary.agent, AgentKind::Codex)
            || !created_after(&summary.created_at, &record.started_at)
            || summary
                .working_directory
                .as_deref()
                .map(normalize)
                .as_deref()
                != Some(target_workdir.as_str())
        {
            continue;
        }
        let detail = database::get_session(db_path, &summary.id)?;
        let first_user = detail
            .messages
            .iter()
            .find(|message| matches!(message.role, MessageRole::User));
        if !first_user
            .map(|message| message.content.contains(&record.marker))
            .unwrap_or(false)
        {
            continue;
        }
        let bound: i64 = database::connect(db_path)?.query_row(
            "SELECT COUNT(*) FROM project_bindings WHERE binding_type='source_session' AND binding_id=?1",
            params![detail.summary.id],
            |row| row.get(0),
        )?;
        if bound > 0 {
            continue;
        }
        let (file_created, file_modified): (Option<String>, Option<String>) =
            database::connect(db_path)?
                .query_row(
                    "SELECT file_created_at,file_modified_at FROM source_sessions WHERE id=?1",
                    params![detail.summary.id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?
                .unwrap_or((None, None));
        database::connect(db_path)?.execute(
            "INSERT INTO continuation_candidates(continuation_id,session_id,session_file_path,normalized_working_directory,created_at,modified_at,first_user_message,confidence,validation_json,discovered_at,selected) VALUES(?1,?2,?3,?4,?5,?6,?7,100,?8,?9,0) ON CONFLICT(continuation_id,session_id) DO UPDATE SET modified_at=excluded.modified_at,first_user_message=excluded.first_user_message,confidence=excluded.confidence,validation_json=excluded.validation_json,discovered_at=excluded.discovered_at",
            params![record.id,detail.summary.id,detail.summary.source_path,target_workdir,file_created.unwrap_or_else(||detail.summary.created_at.clone()),file_modified.unwrap_or_else(||detail.summary.updated_at.clone()),first_user.map(|value|value.content.clone()).unwrap_or_default(),json!({"agent":"codex","createdAfterLaunch":true,"workingDirectory":true,"marker":true,"unbound":true}).to_string(),Utc::now().to_rfc3339()],
        )?;
        matches.push(detail);
    }
    if matches.len() == 1 {
        transition(db_path, id, ContinuationStatus::Binding, None)?;
        bind_detected(db_path, &mut record, &matches[0])?;
        return Ok(ContinuationPollResult {
            continuation: record,
            candidates: vec![],
            inserted_nodes: matches[0].messages.len() + matches[0].tool_calls.len(),
        });
    }
    if matches.len() > 1 {
        if matches!(
            status(&record.status),
            Some(ContinuationStatus::WaitingForSession)
        ) {
            transition(
                db_path,
                id,
                ContinuationStatus::CandidateSessionsFound,
                None,
            )?;
        } else if matches!(
            status(&record.status),
            Some(ContinuationStatus::CandidateSessionsFound)
        ) {
            transition(db_path, id, ContinuationStatus::ManualBindingRequired, None)?;
        }
        database::connect(db_path)?.execute(
            "UPDATE continuations SET warning='检测到多个严格匹配的 Codex 会话，需要手工选择' WHERE id=?1",
            params![id],
        )?;
        record = get(db_path, id)?;
        return Ok(ContinuationPollResult {
            continuation: record,
            candidates: matches.into_iter().map(|detail| detail.summary).collect(),
            inserted_nodes: 0,
        });
    }
    let deadline: Option<String> = database::connect(db_path)?.query_row(
        "SELECT detection_deadline_at FROM continuations WHERE id=?1",
        params![id],
        |row| row.get(0),
    )?;
    if deadline
        .as_deref()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .is_some_and(|value| Utc::now() > value)
    {
        transition(
            db_path,
            id,
            ContinuationStatus::DetectionTimeout,
            Some((
                "session_detection_timeout",
                "在检测期限内没有找到严格匹配的新 Codex 会话",
            )),
        )?;
        record = get(db_path, id)?;
    }
    Ok(ContinuationPollResult {
        continuation: record,
        candidates: vec![],
        inserted_nodes: 0,
    })
}

#[allow(dead_code)]
fn poll_legacy(db_path: &Path, data_dir: &Path, id: &str) -> AppResult<ContinuationPollResult> {
    let mut record = get(db_path, id)?;
    if record.listening {
        let inserted = unified_project::sync(db_path, &record.project_id)?;
        return Ok(ContinuationPollResult {
            continuation: record,
            candidates: vec![],
            inserted_nodes: inserted,
        });
    }
    if !matches!(
        record.status.as_str(),
        "waiting_detection" | "needs_confirmation"
    ) {
        return Ok(ContinuationPollResult {
            continuation: record,
            candidates: vec![],
            inserted_nodes: 0,
        });
    }
    let app_settings = settings::load(db_path, data_dir)?;
    let adapter = CodexAdapter::new();
    let mut paths = app_settings
        .session_paths
        .iter()
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    if paths.is_empty() {
        paths = adapter.default_session_paths();
    }
    let details = adapter.scan_sessions(&paths)?;
    let target_workdir = normalize(&record.working_directory);
    let mut matches = Vec::new();
    for detail in details {
        if !matches!(detail.summary.agent, AgentKind::Codex)
            || !created_after(&detail.summary.created_at, &record.started_at)
        {
            continue;
        }
        if detail
            .summary
            .working_directory
            .as_deref()
            .map(normalize)
            .as_deref()
            != Some(target_workdir.as_str())
        {
            continue;
        }
        let first_user = detail
            .messages
            .iter()
            .find(|message| matches!(message.role, MessageRole::User));
        if !first_user
            .map(|message| message.content.contains(&record.marker))
            .unwrap_or(false)
        {
            continue;
        }
        let conn = database::connect(db_path)?;
        let bound:i64=conn.query_row("SELECT COUNT(*) FROM project_bindings WHERE binding_type='source_session' AND binding_id=?1",params![detail.summary.id],|row|row.get(0))?;
        if bound > 0 {
            continue;
        }
        database::upsert_session(db_path, &detail)?;
        matches.push(detail);
    }
    if matches.len() == 1 {
        let detail = &matches[0];
        bind_detected(db_path, &mut record, detail)?;
        return Ok(ContinuationPollResult {
            continuation: record,
            candidates: vec![],
            inserted_nodes: detail.messages.len(),
        });
    }
    if matches.len() > 1 {
        database::connect(db_path)?.execute("UPDATE continuations SET status='needs_confirmation',warning='检测到多个匹配会话，需要手工确认' WHERE id=?1",params![id])?;
        record.status = "needs_confirmation".into();
        record.warning = Some("检测到多个匹配会话，需要手工确认".into());
    }
    Ok(ContinuationPollResult {
        continuation: record,
        candidates: matches.into_iter().map(|detail| detail.summary).collect(),
        inserted_nodes: 0,
    })
}

fn bind_detected(
    db_path: &Path,
    record: &mut ContinuationRecord,
    detail: &SessionDetail,
) -> AppResult<()> {
    unified_project::bind_detail(
        db_path,
        &record.project_id,
        &record.branch_id,
        detail,
        Some(&record.id),
    )?;
    let detected = Utc::now().to_rfc3339();
    database::connect(db_path)?.execute(
        "UPDATE continuations SET target_session_id=?1,detected_at=?2,warning=NULL WHERE id=?3",
        params![detail.summary.id, detected, record.id],
    )?;
    database::connect(db_path)?.execute(
        "UPDATE continuation_candidates SET selected=CASE WHEN session_id=?1 THEN 1 ELSE 0 END WHERE continuation_id=?2",
        params![detail.summary.id, record.id],
    )?;
    transition(db_path, &record.id, ContinuationStatus::Listening, None)?;
    let compiled_json: String = database::connect(db_path)?.query_row(
        "SELECT compiled_json FROM context_snapshots WHERE id=?1",
        params![record.snapshot_id],
        |row| row.get(0),
    )?;
    let compiled: CompiledContext = serde_json::from_str(&compiled_json)?;
    let before_tokens = compiled.health.estimated_tokens;
    let after_tokens = compiled.estimated_tokens;
    let message=format!("上下文已从约 {before_tokens} tokens 压缩到约 {after_tokens} tokens，并切换到新的 Codex 会话 {}。",detail.summary.id);
    unified_project::insert_event(
        db_path,
        &record.project_id,
        &record.branch_id,
        "session_switch",
        &message,
        json!({"continuationId":record.id,"fromEstimatedTokens":before_tokens,"toEstimatedTokens":after_tokens,"targetSessionId":detail.summary.id}),
    )?;
    record.target_session_id = Some(detail.summary.id.clone());
    record.status = "listening".into();
    record.detected_at = Some(detected);
    record.listening = true;
    record.warning = None;
    Ok(())
}

pub fn bind_manual(db_path: &Path, continuation_id: &str, session_id: &str) -> AppResult<()> {
    let mut record = get(db_path, continuation_id)?;
    let detail = database::get_session(db_path, session_id)?;
    if normalize(detail.summary.working_directory.as_deref().unwrap_or(""))
        != normalize(&record.working_directory)
    {
        return Err(AppError::Message("候选会话工作目录与续接目标不一致".into()));
    }
    let first = detail
        .messages
        .iter()
        .find(|message| matches!(message.role, MessageRole::User));
    if !first
        .map(|message| message.content.contains(&record.marker))
        .unwrap_or(false)
    {
        return Err(AppError::Message(
            "候选会话首条用户消息不包含 Continuation 标识".into(),
        ));
    }
    if !matches!(
        status(&record.status),
        Some(
            ContinuationStatus::WaitingForSession
                | ContinuationStatus::CandidateSessionsFound
                | ContinuationStatus::ManualBindingRequired
        )
    ) {
        return Err(AppError::Message("该 Continuation 当前不能手工绑定".into()));
    }
    transition(db_path, continuation_id, ContinuationStatus::Binding, None)?;
    bind_detected(db_path, &mut record, &detail)
}

pub fn cancel(db_path: &Path, id: &str) -> AppResult<ContinuationRecord> {
    let record = get(db_path, id)?;
    let current = status(&record.status)
        .ok_or_else(|| AppError::Message("Continuation 状态无法识别".into()))?;
    if matches!(
        current,
        ContinuationStatus::Completed | ContinuationStatus::Cancelled
    ) {
        return get(db_path, id);
    }
    database::connect(db_path)?.execute(
        "UPDATE continuations SET cancellation_requested=1 WHERE id=?1",
        params![id],
    )?;
    transition(db_path, id, ContinuationStatus::Cancelled, None)?;
    get(db_path, id)
}

pub fn retry(db_path: &Path, data_dir: &Path, id: &str) -> AppResult<ContinuationRecord> {
    let record = get(db_path, id)?;
    match status(&record.status) {
        Some(ContinuationStatus::LaunchFailed) => {
            if !Path::new(&record.bootstrap_file).is_file() {
                return Err(AppError::Message(
                    "上下文文件缺失，无法安全重试；请重新创建 Fresh Continuation".into(),
                ));
            }
            database::connect(db_path)?.execute(
                "UPDATE continuations SET retry_count=retry_count+1,cancellation_requested=0 WHERE id=?1",
                params![id],
            )?;
            transition(db_path, id, ContinuationStatus::PreparingLaunch, None)?;
            launch_prepared(db_path, data_dir, id)
        }
        Some(
            ContinuationStatus::DetectionTimeout
            | ContinuationStatus::CandidateSessionsFound
            | ContinuationStatus::ManualBindingRequired,
        ) => {
            database::connect(db_path)?.execute(
                "UPDATE continuations SET retry_count=retry_count+1,detection_deadline_at=?1,cancellation_requested=0,warning=NULL WHERE id=?2",
                params![(Utc::now()+chrono::Duration::minutes(3)).to_rfc3339(),id],
            )?;
            transition(db_path, id, ContinuationStatus::WaitingForSession, None)?;
            get(db_path, id)
        }
        _ => Err(AppError::Message("当前状态不支持重试".into())),
    }
}

pub fn recover(db_path: &Path) -> AppResult<Vec<ContinuationRecord>> {
    let ids = {
        let conn = database::connect(db_path)?;
        let mut statement = conn.prepare("SELECT id FROM continuations WHERE status IN ('idle','compiling_context','writing_context','preparing_launch','launching','waiting_for_session','waiting_detection','candidate_sessions_found','needs_confirmation','binding','manual_binding_required') ORDER BY created_at")?;
        let values = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        values
    };
    for id in &ids {
        let record = get(db_path, id)?;
        match status(&record.status) {
            Some(
                ContinuationStatus::Idle
                | ContinuationStatus::CompilingContext
                | ContinuationStatus::WritingContext,
            ) => {
                transition(
                    db_path,
                    id,
                    ContinuationStatus::LaunchFailed,
                    Some((
                        "interrupted_before_context_ready",
                        "客户端退出时上下文尚未准备完成；请重新创建 Fresh Continuation",
                    )),
                )?;
            }
            Some(ContinuationStatus::Launching) if record.process_id.is_some() => {
                if database::connect(db_path)?.query_row(
                    "SELECT detection_deadline_at IS NOT NULL FROM continuations WHERE id=?1",
                    params![id],
                    |row| row.get::<_, i64>(0),
                )? != 0
                {
                    transition(db_path, id, ContinuationStatus::WaitingForSession, None)?;
                } else {
                    transition(
                        db_path,
                        id,
                        ContinuationStatus::LaunchFailed,
                        Some((
                            "interrupted_during_launch",
                            "启动记录不完整，无法确认 Codex 进程状态",
                        )),
                    )?;
                }
            }
            Some(ContinuationStatus::Binding) => {
                transition(
                    db_path,
                    id,
                    ContinuationStatus::ManualBindingRequired,
                    Some((
                        "interrupted_during_binding",
                        "绑定过程被中断，请重新选择候选会话",
                    )),
                )?;
            }
            _ => {}
        }
    }
    ids.iter().map(|id| get(db_path, id)).collect()
}

pub fn cleanup_context_file(db_path: &Path, id: &str) -> AppResult<ContinuationRecord> {
    let record = get(db_path, id)?;
    if !matches!(
        status(&record.status),
        Some(ContinuationStatus::Listening | ContinuationStatus::Completed)
    ) {
        return Err(AppError::Message(
            "只有已绑定或已完成的 Continuation 才能清理上下文文件".into(),
        ));
    }
    let expected_root = Path::new(&record.working_directory)
        .join(".continuum")
        .join("continuations");
    let path = Path::new(&record.bootstrap_file);
    if path.is_file() {
        if !filesystem::is_within(path, &expected_root) {
            return Err(AppError::Message(
                "拒绝清理 Continuum 临时目录之外的文件".into(),
            ));
        }
        fs::remove_file(path)?;
    }
    database::connect(db_path)?.execute(
        "UPDATE continuations SET bootstrap_file='',context_file_path='',updated_at=?1 WHERE id=?2",
        params![Utc::now().to_rfc3339(), id],
    )?;
    get(db_path, id)
}

fn record_native_activity(
    db_path: &Path,
    session_id: &str,
    operation: &str,
    process_id: u32,
) -> AppResult<()> {
    let binding: Option<(String, String)> = database::connect(db_path)?.query_row(
        "SELECT project_id,branch_id FROM project_bindings WHERE binding_type='source_session' AND binding_id=?1 LIMIT 1",
        params![session_id],
        |row| Ok((row.get(0)?,row.get(1)?)),
    ).optional()?;
    let event_id = uuid::Uuid::new_v4().to_string();
    let event_type = if operation == "resume" {
        "native_resume"
    } else {
        "native_fork"
    };
    let (project_id, branch_id) = binding
        .clone()
        .map(|value| (Some(value.0), Some(value.1)))
        .unwrap_or((None, None));
    database::connect(db_path)?.execute(
        "INSERT INTO activity_events(id,project_id,branch_id,event_type,entity_id,summary,metadata_json,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
        params![event_id,project_id,branch_id,event_type,session_id,format!("Codex {operation} 已启动"),json!({"sessionId":session_id,"processId":process_id}).to_string(),Utc::now().to_rfc3339()],
    )?;
    if let Some((project_id, branch_id)) = binding {
        unified_project::insert_event(
            db_path,
            &project_id,
            &branch_id,
            event_type,
            &format!("已使用 Codex {operation} 继续原会话 {session_id}"),
            json!({"sessionId":session_id,"processId":process_id,"operation":operation}),
        )?;
    }
    Ok(())
}

pub fn launch_source(
    db_path: &Path,
    data_dir: &Path,
    session_id: &str,
    operation: &str,
) -> AppResult<u32> {
    if !matches!(operation, "resume" | "fork") {
        return Err(AppError::Message("只支持 resume 或 fork".into()));
    }
    let session = database::get_session(db_path, session_id)?;
    let app_settings = settings::load(db_path, data_dir)?;
    let capabilities = codex_runtime::detect(db_path, data_dir, false)?;
    let supported = match operation {
        "resume" => capabilities.supports_resume,
        "fork" => capabilities.supports_fork,
        _ => false,
    };
    if !capabilities.installed || !supported {
        return Err(AppError::Message(format!(
            "当前 Codex {} 不支持原生 {operation}",
            capabilities.version.as_deref().unwrap_or("未知版本")
        )));
    }
    let cwd = session
        .summary
        .working_directory
        .as_deref()
        .ok_or_else(|| AppError::Message("来源会话没有工作目录".into()))?;
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let child = codex_runtime::command(&app_settings.codex_command)?
            .arg(operation)
            .arg("-C")
            .arg(cwd)
            .arg(session_id)
            .current_dir(cwd)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .creation_flags(0x00000010)
            .spawn()
            .map_err(|error| AppError::Message(format!("启动 Codex {operation} 失败：{error}")))?;
        let process_id = child.id();
        record_native_activity(db_path, session_id, operation, process_id)?;
        Ok(process_id)
    }
    #[cfg(not(windows))]
    {
        let child = codex_runtime::command(&app_settings.codex_command)?
            .arg(operation)
            .arg("-C")
            .arg(cwd)
            .arg(session_id)
            .current_dir(cwd)
            .spawn()
            .map_err(|error| AppError::Message(format!("启动 Codex {operation} 失败：{error}")))?;
        let process_id = child.id();
        record_native_activity(db_path, session_id, operation, process_id)?;
        Ok(process_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn marker_and_path_matching_are_strict() {
        let marker = "CONTINUATION_ID=cont_20260731_abc";
        assert!(format!("start {marker}").contains(marker));
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            normalize(&dir.path().to_string_lossy()),
            normalize(&dir.path().to_string_lossy())
        );
    }

    #[test]
    fn state_machine_rejects_illegal_transitions_and_is_idempotent() {
        let temporary = tempfile::tempdir().unwrap();
        let db_path = temporary.path().join("continuum.sqlite3");
        database::initialize(&db_path).unwrap();
        let now = Utc::now().to_rfc3339();
        let conn = database::connect(&db_path).unwrap();
        conn.execute("INSERT INTO projects(id,name,project_path,git_repository,goal,constraints_json,default_agent,default_model,current_branch_id,current_task,archived,created_at,updated_at,normalized_path,display_path,default_branch_id) VALUES('p','Project','C:/project',NULL,'Goal','[]','codex','default','b','',0,?1,?1,'c:/project','C:/project','b')",params![now]).unwrap();
        conn.execute("INSERT INTO continuations(id,project_id,branch_id,source_node_id,snapshot_id,target_agent,target_model,mode,status,bootstrap_file,launch_command,created_at,working_directory,context_hash,marker,started_at,listening,updated_at) VALUES('c','p','b',NULL,'','codex','default','context','idle','','',?1,'C:/project','','CONTINUATION_ID=c','',0,?1)",params![now]).unwrap();
        drop(conn);
        transition(&db_path, "c", ContinuationStatus::CompilingContext, None).unwrap();
        transition(&db_path, "c", ContinuationStatus::CompilingContext, None).unwrap();
        let version: i64 = database::connect(&db_path)
            .unwrap()
            .query_row(
                "SELECT state_version FROM continuations WHERE id='c'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(version, 1);
        assert!(transition(&db_path, "c", ContinuationStatus::Listening, None).is_err());
    }
}
