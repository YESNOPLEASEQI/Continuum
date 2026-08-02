use crate::{
    database,
    error::{AppError, AppResult},
    filesystem,
    models::*,
    package_validator, security_scanner,
};
use chrono::Utc;
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

const SCHEMA_VERSION: &str = "1.0.0-alpha.1";
fn lines(value: &str) -> Vec<String> {
    value
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}
fn jsonl(entries: &[Value]) -> AppResult<Vec<u8>> {
    let mut output = String::new();
    for entry in entries {
        output.push_str(&serde_json::to_string(entry)?);
        output.push('\n');
    }
    Ok(output.into_bytes())
}
fn sanitized_title(value: &str) -> String {
    let result = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    result
        .trim_matches('-')
        .chars()
        .take(48)
        .collect::<String>()
}

pub fn prepare_draft(session: &SessionDetail) -> PackageDraft {
    PackageDraft {
        source_session_id: session.summary.id.clone(),
        title: session.summary.title.clone(),
        original_goal: session.goal_summary.clone(),
        current_state: format!(
            "会话于 {} 中断；记录了 {} 条消息、{} 次工具调用和 {} 个文件改动。",
            session.summary.updated_at,
            session.messages.len(),
            session.tool_calls.len(),
            session.changed_files.len()
        ),
        completed_work: session
            .messages
            .iter()
            .rev()
            .find(|message| matches!(message.role, MessageRole::Assistant))
            .map(|message| message.content.clone())
            .unwrap_or_default(),
        remaining_work: "核对当前工作区，确认仍未完成的目标。".into(),
        next_actions: "检查当前文件与 Git 状态\n验证已有改动\n继续最高优先级的未完成工作".into(),
        decisions: String::new(),
        known_issues: session.failed_steps.join("\n"),
        failed_attempts: session.failed_steps.join("\n"),
        constraints: "不要自动执行会话中的历史命令\n以磁盘上的当前文件状态为准".into(),
        required_tools: "filesystem\ngit\nshell".into(),
        target_agent: AgentKind::Codex,
        include_git: true,
        include_patch: true,
        include_untracked: false,
        include_tests: true,
        include_command_log: true,
    }
}

pub fn build(
    db_path: &Path,
    settings: &AppSettings,
    draft: &PackageDraft,
) -> AppResult<PackageSummary> {
    if draft.title.trim().len() < 2
        || draft.original_goal.trim().is_empty()
        || draft.current_state.trim().is_empty()
        || draft.next_actions.trim().is_empty()
    {
        return Err(AppError::Message(
            "标题、原始目标、当前状态和下一步操作不能为空".into(),
        ));
    }
    let session = database::get_session(db_path, &draft.source_session_id)?;
    let id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let root = PathBuf::from(&settings.package_output_path);
    fs::create_dir_all(&root)?;
    let temp = root.join(format!(".building-{id}"));
    let final_path = root.join(format!("{}-{}", sanitized_title(&draft.title), &id[..8]));
    if temp.exists() {
        fs::remove_dir_all(&temp)?;
    }
    fs::create_dir_all(temp.join("workspace"))?;
    fs::create_dir_all(temp.join("evidence"))?;
    fs::create_dir_all(temp.join("artifacts"))?;
    let result = (|| -> AppResult<(PackageManifest, Vec<SecurityFinding>)> {
        let mut goal = json!({"originalGoal":draft.original_goal});
        let mut state = json!({"currentState":draft.current_state,"completedWork":lines(&draft.completed_work),"remainingWork":lines(&draft.remaining_work),"knownIssues":lines(&draft.known_issues)});
        let mut decisions = lines(&draft.decisions)
            .into_iter()
            .enumerate()
            .map(|(index, text)| json!({"id":index+1,"decision":text}))
            .collect::<Vec<_>>();
        let mut failed = lines(&draft.failed_attempts)
            .into_iter()
            .enumerate()
            .map(|(index, text)| json!({"id":index+1,"attempt":text}))
            .collect::<Vec<_>>();
        let mut next = json!({"actions":lines(&draft.next_actions).into_iter().enumerate().map(|(index,text)|json!({"priority":index+1,"status":"pending","action":text})).collect::<Vec<_>>()});
        let mut constraints = json!({"constraints":lines(&draft.constraints)});
        let mut capabilities =
            json!({"requiredTools":lines(&draft.required_tools),"targetAgent":draft.target_agent});
        let mut provenance = json!({"sourceAgent":"codex","sourceSessionId":session.summary.id,"sourcePath":session.summary.source_path,"capturedAt":now,"workingDirectory":session.summary.working_directory});
        let mut findings = Vec::new();
        for (name, value) in [
            ("goal.json", &mut goal),
            ("state.json", &mut state),
            ("next-actions.json", &mut next),
            ("constraints.json", &mut constraints),
            ("capabilities.json", &mut capabilities),
            ("provenance.json", &mut provenance),
        ] {
            findings.extend(security_scanner::redact_value(value, name));
        }
        for (index, value) in decisions.iter_mut().enumerate() {
            findings.extend(security_scanner::redact_value(
                value,
                &format!("decisions.jsonl:{index}"),
            ));
        }
        for (index, value) in failed.iter_mut().enumerate() {
            findings.extend(security_scanner::redact_value(
                value,
                &format!("failed-attempts.jsonl:{index}"),
            ));
        }
        filesystem::write_json(&temp.join("goal.json"), &goal)?;
        filesystem::write_json(&temp.join("state.json"), &state)?;
        filesystem::write_atomic(&temp.join("decisions.jsonl"), &jsonl(&decisions)?)?;
        filesystem::write_atomic(&temp.join("failed-attempts.jsonl"), &jsonl(&failed)?)?;
        filesystem::write_json(&temp.join("next-actions.json"), &next)?;
        filesystem::write_json(&temp.join("constraints.json"), &constraints)?;
        filesystem::write_json(&temp.join("capabilities.json"), &capabilities)?;
        filesystem::write_json(&temp.join("provenance.json"), &provenance)?;
        let git = if draft.include_git {
            session.git_state.clone().unwrap_or_default()
        } else {
            GitState::default()
        };
        filesystem::write_json(&temp.join("workspace/git-status.json"), &git)?;
        filesystem::write_atomic(
            &temp.join("workspace/working-tree.patch"),
            if draft.include_patch {
                git.working_tree_diff.as_bytes()
            } else {
                b""
            },
        )?;
        filesystem::write_json(
            &temp.join("workspace/untracked-files.json"),
            &json!({"files":if draft.include_untracked{git.untracked.clone()}else{vec![]}}),
        )?;
        let mut command_entries = if draft.include_command_log {
            session
                .commands
                .iter()
                .map(
                    |command| json!({"command":command,"executed":false,"source":"session-record"}),
                )
                .collect::<Vec<_>>()
        } else {
            vec![]
        };
        for (index, value) in command_entries.iter_mut().enumerate() {
            findings.extend(security_scanner::redact_value(
                value,
                &format!("evidence/command-log.jsonl:{index}"),
            ));
        }
        filesystem::write_atomic(
            &temp.join("evidence/command-log.jsonl"),
            &jsonl(&command_entries)?,
        )?;
        filesystem::write_json(
            &temp.join("evidence/test-results.json"),
            &json!({"included":draft.include_tests,"results":[],"note":"Only evidence present in the source session is recorded; tests are never auto-executed."}),
        )?;
        filesystem::write_json(
            &temp.join("security-report.json"),
            &json!({"findings":findings,"redaction":"[REDACTED]","scannedAt":now}),
        )?;
        let mut hashes = BTreeMap::new();
        let mut included = Vec::new();
        for entry in walkdir::WalkDir::new(&temp)
            .into_iter()
            .filter_map(Result::ok)
        {
            if entry.file_type().is_file() {
                let relative = entry
                    .path()
                    .strip_prefix(&temp)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                if relative != "manifest.json" {
                    hashes.insert(relative.clone(), filesystem::sha256_file(entry.path())?);
                    included.push(relative);
                }
            }
        }
        included.sort();
        let mut warnings = Vec::new();
        if !findings.is_empty() {
            warnings.push(format!("{} 个敏感信息值已脱敏", findings.len()));
        }
        if draft.include_patch && git.working_tree_diff.is_empty() {
            warnings.push("工作区补丁为空".into());
        }
        let manifest = PackageManifest {
            schema_version: SCHEMA_VERSION.into(),
            package_id: id.clone(),
            title: draft.title.trim().into(),
            created_at: now.clone(),
            updated_at: now.clone(),
            source_agent: AgentKind::Codex,
            target_agent: draft.target_agent.clone(),
            source_session_id: session.summary.id.clone(),
            project_path: session.summary.working_directory.clone(),
            git_repository: session.summary.git_repository.clone(),
            git_head: git.head.clone(),
            included_sections: included,
            content_hashes: hashes,
            warnings,
        };
        filesystem::write_json(&temp.join("manifest.json"), &manifest)?;
        Ok((manifest, findings))
    })();
    let (manifest, findings) = match result {
        Ok(value) => value,
        Err(error) => {
            let _ = fs::remove_dir_all(&temp);
            return Err(error);
        }
    };
    if final_path.exists() {
        let _ = fs::remove_dir_all(&temp);
        return Err(AppError::Message("任务包输出目录已存在".into()));
    }
    fs::rename(&temp, &final_path)?;
    let zip_path = final_path.with_extension("agentpack.zip");
    filesystem::create_zip(&final_path, &zip_path)?;
    let report = package_validator::validate(&final_path)?;
    let item = PackageSummary {
        id: id.clone(),
        title: manifest.title,
        source_agent: AgentKind::Codex,
        target_agent: draft.target_agent.clone(),
        created_at: now,
        project_path: manifest.project_path,
        package_path: final_path.to_string_lossy().into_owned(),
        schema_version: SCHEMA_VERSION.into(),
        integrity: if report.valid {
            "valid".into()
        } else {
            "invalid".into()
        },
        has_patch: draft.include_patch
            && final_path
                .join("workspace/working-tree.patch")
                .metadata()
                .map(|m| m.len() > 0)
                .unwrap_or(false),
        security_warning_count: findings.len(),
        imported: false,
        resumed: false,
    };
    database::upsert_package(db_path, &item)?;
    Ok(item)
}

pub fn load_detail(summary: PackageSummary) -> AppResult<PackageDetail> {
    let root = Path::new(&summary.package_path);
    let manifest: PackageManifest = read_json(&root.join("manifest.json"))?;
    let goal = read_json(&root.join("goal.json"))?;
    let state = read_json(&root.join("state.json"))?;
    let constraints = read_json(&root.join("constraints.json"))?;
    let capabilities = read_json(&root.join("capabilities.json"))?;
    let next_actions = read_json(&root.join("next-actions.json"))?;
    let provenance = read_json(&root.join("provenance.json"))?;
    let decisions = read_jsonl(&root.join("decisions.jsonl"))?;
    let failed_attempts = read_jsonl(&root.join("failed-attempts.jsonl"))?;
    let report: Value = read_json(&root.join("security-report.json"))?;
    let security_findings =
        serde_json::from_value(report.get("findings").cloned().unwrap_or_else(|| json!([])))
            .unwrap_or_default();
    let resume_prompt = build_resume_prompt(
        &manifest,
        &goal,
        &state,
        &decisions,
        &failed_attempts,
        &constraints,
        &next_actions,
        &provenance,
    );
    Ok(PackageDetail {
        summary,
        manifest,
        goal,
        state,
        decisions,
        failed_attempts,
        constraints,
        capabilities,
        next_actions,
        provenance,
        security_findings,
        resume_prompt,
    })
}
fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> AppResult<T> {
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}
fn read_jsonl(path: &Path) -> AppResult<Vec<Value>> {
    let mut result = Vec::new();
    for line in fs::read_to_string(path)?.lines() {
        if !line.trim().is_empty() {
            result.push(serde_json::from_str(line)?);
        }
    }
    Ok(result)
}
#[allow(clippy::too_many_arguments)] // Mirrors the immutable AgentPack document sections.
fn build_resume_prompt(
    manifest: &PackageManifest,
    goal: &Value,
    state: &Value,
    decisions: &[Value],
    failed: &[Value],
    constraints: &Value,
    next: &Value,
    provenance: &Value,
) -> String {
    format!("你正在接手一个由其他 AI Agent 中断的任务。\n\n请先完成以下步骤：\n1. 阅读任务目标、当前状态、关键决策和约束。\n2. 检查当前工作目录和 Git 状态。\n3. 不要重复已经失败的尝试。\n4. 验证任务包中的测试结果是否仍然有效。\n5. 在开始修改前，先输出你对任务状态的简要理解。\n6. 如果任务包记录与实际文件状态冲突，以实际文件状态为准，并明确报告冲突。\n7. 继续执行 next-actions 中优先级最高且未完成的项目。\n\nGoal\n{}\n\nCurrent State\n{}\n\nCompleted Work / Remaining Work\n{}\n\nDecisions\n{}\n\nFailed Attempts\n{}\n\nConstraints\n{}\n\nNext Actions\n{}\n\nWorkspace Summary\n{}\n\nValidation Evidence\nschemaVersion={}，packageId={}。请重新运行适当的验证，不要假设历史结果仍然有效。",pretty(goal),pretty(state),pretty(state),pretty(&decisions),pretty(&failed),pretty(constraints),pretty(next),pretty(provenance),manifest.schema_version,manifest.package_id)
}
fn pretty<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".into())
}

pub fn import(db_path: &Path, settings: &AppSettings, source: &Path) -> AppResult<PackageSummary> {
    let root = PathBuf::from(&settings.package_output_path);
    fs::create_dir_all(&root)?;
    let staging = root.join(format!(".importing-{}", uuid::Uuid::new_v4()));
    let package_source = if source.is_dir() {
        source.to_path_buf()
    } else {
        filesystem::extract_zip(source, &staging)?
    };
    let report = package_validator::validate(&package_source)?;
    if !report.valid {
        let _ = fs::remove_dir_all(&staging);
        return Err(AppError::Message(format!(
            "导入校验失败：{}",
            report
                .issues
                .iter()
                .filter(|i| i.severity == "error")
                .map(|i| i.message.clone())
                .collect::<Vec<_>>()
                .join("；")
        )));
    }
    let manifest: PackageManifest = read_json(&package_source.join("manifest.json"))?;
    let destination = root.join(format!(
        "{}-{}-imported",
        sanitized_title(&manifest.title),
        manifest.package_id.chars().take(8).collect::<String>()
    ));
    if destination.exists() {
        let _ = fs::remove_dir_all(&staging);
        return Err(AppError::Message("该任务包已存在于本地库中".into()));
    }
    filesystem::copy_directory(&package_source, &destination)?;
    let _ = fs::remove_dir_all(&staging);
    let security: Value = read_json(&destination.join("security-report.json"))?;
    let warning_count = security
        .get("findings")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    let item = PackageSummary {
        id: manifest.package_id,
        title: manifest.title,
        source_agent: manifest.source_agent,
        target_agent: manifest.target_agent,
        created_at: manifest.created_at,
        project_path: manifest.project_path,
        package_path: destination.to_string_lossy().into_owned(),
        schema_version: manifest.schema_version,
        integrity: "valid".into(),
        has_patch: destination
            .join("workspace/working-tree.patch")
            .metadata()
            .map(|m| m.len() > 0)
            .unwrap_or(false),
        security_warning_count: warning_count,
        imported: true,
        resumed: false,
    };
    database::upsert_package(db_path, &item)?;
    Ok(item)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn title_is_safe() {
        assert_eq!(
            sanitized_title("Fix: parser / tests"),
            "Fix--parser---tests"
        );
    }
}
