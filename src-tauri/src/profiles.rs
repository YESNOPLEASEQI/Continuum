use crate::{
    codex_runtime, database,
    error::{AppError, AppResult},
    filesystem,
    models::{CodexCapabilityReport, CodexProfile},
};
use rusqlite::{params, OptionalExtension};
use std::{path::Path, time::Duration};

const DEFAULT_PROMPT: &str = "你正在继续一个由旧会话压缩而来的项目任务。请先读取 {{CONTEXT_FILE_PATH}}，核对当前工作目录、Git 和实际文件；如有冲突以实际文件为准，然后复述目标并继续最高优先级任务。{{CONTINUATION_MARKER}}";

fn bool_value(value: i64) -> bool {
    value != 0
}

fn row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CodexProfile> {
    let arguments: String = row.get(8)?;
    Ok(CodexProfile {
        id: row.get(0)?,
        project_id: row.get(1)?,
        branch_id: row.get(2)?,
        name: row.get(3)?,
        executable_path: row.get(4)?,
        model: row.get(5)?,
        working_directory: row.get(6)?,
        approval_mode: row.get(7)?,
        sandbox_mode: row.get(9)?,
        launch_arguments: serde_json::from_str(&arguments).unwrap_or_default(),
        context_budget: row.get::<_, i64>(10)? as usize,
        recent_message_limit: row.get::<_, i64>(11)? as usize,
        include_git_status: bool_value(row.get(12)?),
        include_git_diff: bool_value(row.get(13)?),
        include_tests: bool_value(row.get(14)?),
        include_failed_attempts: bool_value(row.get(15)?),
        include_skills: bool_value(row.get(16)?),
        include_mcp: bool_value(row.get(17)?),
        launch_prompt_template: row.get(18)?,
        created_at: row.get(19)?,
        updated_at: row.get(20)?,
    })
}

const SELECT: &str = "SELECT id,project_id,branch_id,name,executable_path,model,working_directory,approval_mode,launch_arguments_json,sandbox_mode,context_budget,recent_message_limit,include_git_status,include_git_diff,include_tests,include_failed_attempts,include_skills,include_mcp,launch_prompt_template,created_at,updated_at FROM codex_profiles";

pub fn default_profile(
    project_id: Option<String>,
    branch_id: Option<String>,
    working_directory: String,
    capabilities: &CodexCapabilityReport,
    context_budget: usize,
) -> CodexProfile {
    let now = chrono::Utc::now().to_rfc3339();
    CodexProfile {
        id: uuid::Uuid::new_v4().to_string(),
        project_id,
        branch_id,
        name: "Default Codex".into(),
        executable_path: capabilities
            .executable_path
            .clone()
            .unwrap_or_else(|| "codex".into()),
        model: None,
        working_directory,
        approval_mode: "on-request".into(),
        sandbox_mode: "workspace-write".into(),
        launch_arguments: vec![],
        context_budget,
        recent_message_limit: 24,
        include_git_status: true,
        include_git_diff: false,
        include_tests: true,
        include_failed_attempts: true,
        include_skills: true,
        include_mcp: true,
        launch_prompt_template: DEFAULT_PROMPT.into(),
        created_at: now.clone(),
        updated_at: now,
    }
}

pub fn validate(profile: &CodexProfile, capabilities: &CodexCapabilityReport) -> AppResult<()> {
    if profile.name.trim().is_empty() {
        return Err(AppError::Message("Profile 名称不能为空".into()));
    }
    if !Path::new(&profile.working_directory).is_dir() {
        return Err(AppError::Message("Profile 工作目录不存在".into()));
    }
    if !matches!(
        profile.approval_mode.as_str(),
        "untrusted" | "on-request" | "never"
    ) {
        return Err(AppError::Message(
            "当前 Codex Profile 的 Approval Mode 无效".into(),
        ));
    }
    if !matches!(
        profile.sandbox_mode.as_str(),
        "read-only" | "workspace-write" | "danger-full-access"
    ) {
        return Err(AppError::Message(
            "当前 Codex Profile 的 Sandbox Mode 无效".into(),
        ));
    }
    if !capabilities.supports_approval && profile.approval_mode != "on-request" {
        return Err(AppError::Message(
            "当前 Codex 版本未检测到 Approval 参数".into(),
        ));
    }
    if !capabilities.supports_sandbox && profile.sandbox_mode != "workspace-write" {
        return Err(AppError::Message(
            "当前 Codex 版本未检测到 Sandbox 参数".into(),
        ));
    }
    if profile.model.is_some() && !capabilities.supports_model {
        return Err(AppError::Message(
            "当前 Codex 版本未检测到 Model 参数".into(),
        ));
    }
    if !(1_000..=1_000_000).contains(&profile.context_budget) {
        return Err(AppError::Message(
            "Context Budget 必须在 1,000 到 1,000,000 之间".into(),
        ));
    }
    if !(1..=500).contains(&profile.recent_message_limit) {
        return Err(AppError::Message("最近消息数量必须在 1 到 500 之间".into()));
    }
    if !profile
        .launch_prompt_template
        .contains("{{CONTEXT_FILE_PATH}}")
        || !profile
            .launch_prompt_template
            .contains("{{CONTINUATION_MARKER}}")
    {
        return Err(AppError::Message(
            "启动模板必须包含 CONTEXT_FILE_PATH 和 CONTINUATION_MARKER 占位符".into(),
        ));
    }
    for argument in &profile.launch_arguments {
        if !argument.starts_with('-') {
            return Err(AppError::Message(format!(
                "不允许位置参数：{argument}；启动上下文由 Continuum 单独注入"
            )));
        }
        if argument.contains("dangerously-bypass") {
            return Err(AppError::Message(
                "Profile 不允许绕过 Codex 安全策略".into(),
            ));
        }
        let known = matches!(
            argument.as_str(),
            "--search" | "--no-alt-screen" | "--strict-config"
        ) || argument.starts_with("--enable=")
            || argument.starts_with("--disable=")
            || argument.starts_with("--config=")
            || argument.starts_with("-c=")
            || (capabilities.supports_profile
                && (argument.starts_with("--profile=") || argument.starts_with("-p=")));
        if !known {
            return Err(AppError::Message(format!(
                "启动参数未通过当前 Codex 能力白名单：{argument}"
            )));
        }
    }
    let version = codex_runtime::output_with_timeout(
        &profile.executable_path,
        &["--version"],
        Duration::from_secs(10),
    )?;
    if !version.status.success() {
        return Err(AppError::Message(
            "Profile 指定的 Codex 可执行文件不可用".into(),
        ));
    }
    Ok(())
}

pub fn save(
    db_path: &Path,
    mut profile: CodexProfile,
    capabilities: &CodexCapabilityReport,
) -> AppResult<CodexProfile> {
    validate(&profile, capabilities)?;
    let now = chrono::Utc::now().to_rfc3339();
    if profile.id.trim().is_empty() {
        profile.id = uuid::Uuid::new_v4().to_string();
        profile.created_at = now.clone();
    }
    if profile.created_at.trim().is_empty() {
        profile.created_at = now.clone();
    }
    profile.updated_at = now;
    database::connect(db_path)?.execute("INSERT INTO codex_profiles(id,project_id,branch_id,name,executable_path,model,working_directory,approval_mode,sandbox_mode,launch_arguments_json,context_budget,recent_message_limit,include_git_status,include_git_diff,include_tests,include_failed_attempts,include_skills,include_mcp,launch_prompt_template,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21) ON CONFLICT(id) DO UPDATE SET project_id=excluded.project_id,branch_id=excluded.branch_id,name=excluded.name,executable_path=excluded.executable_path,model=excluded.model,working_directory=excluded.working_directory,approval_mode=excluded.approval_mode,sandbox_mode=excluded.sandbox_mode,launch_arguments_json=excluded.launch_arguments_json,context_budget=excluded.context_budget,recent_message_limit=excluded.recent_message_limit,include_git_status=excluded.include_git_status,include_git_diff=excluded.include_git_diff,include_tests=excluded.include_tests,include_failed_attempts=excluded.include_failed_attempts,include_skills=excluded.include_skills,include_mcp=excluded.include_mcp,launch_prompt_template=excluded.launch_prompt_template,updated_at=excluded.updated_at",params![profile.id,profile.project_id,profile.branch_id,profile.name,profile.executable_path,profile.model,profile.working_directory,profile.approval_mode,profile.sandbox_mode,serde_json::to_string(&profile.launch_arguments)?,profile.context_budget as i64,profile.recent_message_limit as i64,profile.include_git_status,profile.include_git_diff,profile.include_tests,profile.include_failed_attempts,profile.include_skills,profile.include_mcp,profile.launch_prompt_template,profile.created_at,profile.updated_at])?;
    get(db_path, &profile.id)
}

pub fn get(db_path: &Path, id: &str) -> AppResult<CodexProfile> {
    database::connect(db_path)?
        .query_row(&format!("{SELECT} WHERE id=?1"), params![id], row)
        .optional()?
        .ok_or_else(|| AppError::Message("找不到 Codex Profile".into()))
}

pub fn list(db_path: &Path, project_id: Option<&str>) -> AppResult<Vec<CodexProfile>> {
    let conn = database::connect(db_path)?;
    let sql = if project_id.is_some() {
        format!(
            "{SELECT} WHERE project_id=?1 OR project_id IS NULL ORDER BY project_id IS NULL,name"
        )
    } else {
        format!("{SELECT} ORDER BY name")
    };
    let mut statement = conn.prepare(&sql)?;
    let rows = if let Some(project_id) = project_id {
        statement
            .query_map(params![project_id], row)?
            .collect::<Result<Vec<_>, _>>()?
    } else {
        statement
            .query_map([], row)?
            .collect::<Result<Vec<_>, _>>()?
    };
    Ok(rows)
}

pub fn resolve(
    db_path: &Path,
    project_id: &str,
    branch_id: &str,
) -> AppResult<Option<CodexProfile>> {
    let conn = database::connect(db_path)?;
    let branch_profile: Option<String> = conn
        .query_row(
            "SELECT binding_id FROM project_bindings WHERE project_id=?1 AND binding_type='branch_codex_profile' AND branch_id=?2 LIMIT 1",
            params![project_id, branch_id],
            |row| row.get(0),
        )
        .optional()?;
    let project_profile: Option<String> = conn
        .query_row(
            "SELECT default_codex_profile_id FROM projects WHERE id=?1",
            params![project_id],
            |row| row.get(0),
        )
        .optional()?
        .flatten();
    let global_profile: Option<String> = conn
        .query_row(
            "SELECT id FROM codex_profiles WHERE project_id IS NULL ORDER BY updated_at DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()?;
    drop(conn);
    branch_profile
        .or(project_profile)
        .or(global_profile)
        .map(|id| get(db_path, &id))
        .transpose()
}

pub fn duplicate(db_path: &Path, id: &str, name: &str) -> AppResult<CodexProfile> {
    let mut profile = get(db_path, id)?;
    profile.id = uuid::Uuid::new_v4().to_string();
    profile.name = name.trim().to_owned();
    profile.created_at = chrono::Utc::now().to_rfc3339();
    profile.updated_at = profile.created_at.clone();
    database::connect(db_path)?.execute("INSERT INTO codex_profiles(id,project_id,branch_id,name,executable_path,model,working_directory,approval_mode,sandbox_mode,launch_arguments_json,context_budget,recent_message_limit,include_git_status,include_git_diff,include_tests,include_failed_attempts,include_skills,include_mcp,launch_prompt_template,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21)",params![profile.id,profile.project_id,profile.branch_id,profile.name,profile.executable_path,profile.model,profile.working_directory,profile.approval_mode,profile.sandbox_mode,serde_json::to_string(&profile.launch_arguments)?,profile.context_budget as i64,profile.recent_message_limit as i64,profile.include_git_status,profile.include_git_diff,profile.include_tests,profile.include_failed_attempts,profile.include_skills,profile.include_mcp,profile.launch_prompt_template,profile.created_at,profile.updated_at])?;
    get(db_path, &profile.id)
}

pub fn delete(db_path: &Path, id: &str) -> AppResult<()> {
    let mut conn = database::connect(db_path)?;
    let transaction = conn.transaction()?;
    transaction.execute(
        "UPDATE projects SET default_codex_profile_id=NULL WHERE default_codex_profile_id=?1",
        params![id],
    )?;
    transaction.execute(
        "DELETE FROM project_bindings WHERE binding_type='branch_codex_profile' AND binding_id=?1",
        params![id],
    )?;
    let changed = transaction.execute("DELETE FROM codex_profiles WHERE id=?1", params![id])?;
    if changed == 0 {
        return Err(AppError::Message("找不到 Codex Profile".into()));
    }
    transaction.commit()?;
    Ok(())
}

pub fn set_project_default(db_path: &Path, project_id: &str, profile_id: &str) -> AppResult<()> {
    let profile = get(db_path, profile_id)?;
    if profile
        .project_id
        .as_deref()
        .is_some_and(|value| value != project_id)
    {
        return Err(AppError::Message("该 Profile 属于另一个项目".into()));
    }
    database::connect(db_path)?.execute(
        "UPDATE projects SET default_codex_profile_id=?1,updated_at=?2 WHERE id=?3",
        params![profile_id, chrono::Utc::now().to_rfc3339(), project_id],
    )?;
    Ok(())
}

pub fn set_branch_default(
    db_path: &Path,
    project_id: &str,
    branch_id: &str,
    profile_id: &str,
) -> AppResult<()> {
    let _ = get(db_path, profile_id)?;
    let mut conn = database::connect(db_path)?;
    let transaction = conn.transaction()?;
    transaction.execute(
        "DELETE FROM project_bindings WHERE project_id=?1 AND binding_type='branch_codex_profile' AND branch_id=?2",
        params![project_id, branch_id],
    )?;
    transaction.execute("INSERT INTO project_bindings(project_id,binding_type,binding_id,branch_id,created_at,metadata_json) VALUES(?1,'branch_codex_profile',?2,?3,?4,'{}')",params![project_id,profile_id,branch_id,chrono::Utc::now().to_rfc3339()])?;
    transaction.commit()?;
    Ok(())
}

pub fn export_profile(db_path: &Path, id: &str, path: &Path) -> AppResult<String> {
    let profile = get(db_path, id)?;
    filesystem::write_json(path, &profile)?;
    Ok(path.to_string_lossy().into_owned())
}

pub fn import_profile(
    db_path: &Path,
    path: &Path,
    capabilities: &CodexCapabilityReport,
) -> AppResult<CodexProfile> {
    let mut profile: CodexProfile = serde_json::from_str(&std::fs::read_to_string(path)?)?;
    profile.id = uuid::Uuid::new_v4().to_string();
    profile.name = format!("{}（导入）", profile.name);
    profile.created_at = String::new();
    profile.updated_at = String::new();
    save(db_path, profile, capabilities)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn capabilities() -> CodexCapabilityReport {
        CodexCapabilityReport {
            capability_schema_version: 2,
            installed: true,
            executable_path: Some("codex".into()),
            version: Some("codex-cli test".into()),
            help_hash: Some("hash".into()),
            supports_resume: true,
            supports_fork: true,
            supports_cd: true,
            supports_model: true,
            supports_profile: true,
            supports_sandbox: true,
            supports_approval: true,
            supports_app_server: true,
            session_paths: vec![],
            checked_at: "now".into(),
            error: None,
        }
    }

    #[test]
    fn rejects_unknown_and_unsafe_launch_arguments_before_executable_check() {
        let temporary = tempfile::tempdir().unwrap();
        let mut profile = default_profile(
            None,
            None,
            temporary.path().to_string_lossy().into_owned(),
            &capabilities(),
            32_000,
        );
        profile.launch_arguments = vec!["--dangerously-bypass-approvals-and-sandbox".into()];
        assert!(validate(&profile, &capabilities()).is_err());
        profile.launch_arguments = vec!["unverified-positional-value".into()];
        assert!(validate(&profile, &capabilities()).is_err());
    }
}
