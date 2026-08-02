use crate::{
    error::{AppError, AppResult},
    filesystem, git_inspector,
    models::*,
    security_scanner,
};
use chrono::Utc;
use std::{fs, path::Path};

const REQUIRED: &[&str] = &[
    "manifest.json",
    "goal.json",
    "state.json",
    "decisions.jsonl",
    "failed-attempts.jsonl",
    "next-actions.json",
    "constraints.json",
    "capabilities.json",
    "provenance.json",
    "security-report.json",
    "workspace/git-status.json",
    "workspace/working-tree.patch",
    "workspace/untracked-files.json",
    "evidence/command-log.jsonl",
    "evidence/test-results.json",
];

fn issue(
    code: &str,
    message: impl Into<String>,
    path: Option<String>,
    severity: &str,
) -> ValidationIssue {
    ValidationIssue {
        code: code.into(),
        message: message.into(),
        path,
        severity: severity.into(),
    }
}

pub fn validate(package_path: &Path) -> AppResult<ValidationReport> {
    if !package_path.is_dir() {
        return Err(AppError::Message("任务包目录不存在".into()));
    }
    let mut issues = Vec::new();
    for relative in REQUIRED {
        if !package_path.join(relative).is_file() {
            issues.push(issue(
                "missing_file",
                format!("缺少必需文件 {relative}"),
                Some((*relative).into()),
                "error",
            ));
        }
    }
    let manifest_path = package_path.join("manifest.json");
    let manifest: Option<PackageManifest> = if manifest_path.is_file() {
        match fs::read_to_string(&manifest_path)
            .and_then(|text| serde_json::from_str(&text).map_err(std::io::Error::other))
        {
            Ok(value) => Some(value),
            Err(error) => {
                issues.push(issue(
                    "invalid_manifest",
                    error.to_string(),
                    Some("manifest.json".into()),
                    "error",
                ));
                None
            }
        }
    } else {
        None
    };
    if let Some(manifest) = &manifest {
        if manifest.schema_version != "1.0.0-alpha.1" {
            issues.push(issue(
                "schema_version",
                format!("不支持的 schemaVersion {}", manifest.schema_version),
                Some("manifest.json".into()),
                "error",
            ));
        }
        for (relative, expected) in &manifest.content_hashes {
            let path = package_path.join(relative);
            if path.is_file() {
                match filesystem::sha256_file(&path) {
                    Ok(actual) if &actual != expected => issues.push(issue(
                        "hash_mismatch",
                        format!("SHA-256 不一致：{relative}"),
                        Some(relative.clone()),
                        "error",
                    )),
                    Err(error) => issues.push(issue(
                        "hash_read_failed",
                        error.to_string(),
                        Some(relative.clone()),
                        "error",
                    )),
                    _ => {}
                }
            } else {
                issues.push(issue(
                    "hashed_file_missing",
                    format!("哈希清单文件不存在：{relative}"),
                    Some(relative.clone()),
                    "error",
                ));
            }
        }
        if let Some(project) = manifest.project_path.as_deref() {
            if !Path::new(project).exists() {
                issues.push(issue(
                    "project_path_missing",
                    "记录的项目路径当前不存在",
                    Some(project.into()),
                    "warning",
                ));
            } else if let Some(expected) = manifest.git_head.as_deref() {
                let current = git_inspector::inspect(Path::new(project));
                if current.head.as_deref() != Some(expected) {
                    issues.push(issue(
                        "git_head_changed",
                        "当前 Git HEAD 与任务包记录不一致",
                        Some(project.into()),
                        "warning",
                    ));
                }
            }
        }
    }
    for relative in [
        "decisions.jsonl",
        "failed-attempts.jsonl",
        "evidence/command-log.jsonl",
    ] {
        let path = package_path.join(relative);
        if path.is_file() {
            for (index, line) in fs::read_to_string(&path)?.lines().enumerate() {
                if !line.trim().is_empty()
                    && serde_json::from_str::<serde_json::Value>(line).is_err()
                {
                    issues.push(issue(
                        "invalid_jsonl",
                        format!("第 {} 行无法解析", index + 1),
                        Some(relative.into()),
                        "error",
                    ));
                }
            }
        }
    }
    let patch = package_path.join("workspace/working-tree.patch");
    if patch.is_file() && fs::metadata(&patch)?.len() == 0 {
        issues.push(issue(
            "empty_patch",
            "工作区补丁为空",
            Some("workspace/working-tree.patch".into()),
            "warning",
        ));
    }
    for entry in walkdir::WalkDir::new(package_path)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(package_path)
            .unwrap_or(entry.path())
            .to_string_lossy()
            .replace('\\', "/");
        if relative.ends_with(".zip") {
            continue;
        }
        let text = fs::read_to_string(entry.path()).unwrap_or_default();
        let (_, findings) = security_scanner::redact_text(&text, &relative, "$");
        if !findings.is_empty() {
            issues.push(issue(
                "sensitive_information",
                format!("发现 {} 个可能的敏感值", findings.len()),
                Some(relative.clone()),
                "error",
            ));
        }
        if text.contains("C:\\Users\\") || text.contains("/Users/") || text.contains("/home/") {
            issues.push(issue(
                "absolute_path",
                "文件内容包含绝对用户路径",
                Some(relative),
                "warning",
            ));
        }
    }
    let valid = !issues.iter().any(|item| item.severity == "error");
    Ok(ValidationReport {
        valid,
        checked_at: Utc::now().to_rfc3339(),
        issues,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    fn minimal(dir: &Path) {
        for path in REQUIRED {
            let full = dir.join(path);
            fs::create_dir_all(full.parent().unwrap()).unwrap();
            fs::write(full, if path.ends_with(".json") { "{}" } else { "" }).unwrap();
        }
        let manifest = PackageManifest {
            schema_version: "1.0.0-alpha.1".into(),
            package_id: "p1".into(),
            title: "test".into(),
            created_at: Utc::now().to_rfc3339(),
            updated_at: Utc::now().to_rfc3339(),
            source_agent: AgentKind::Codex,
            target_agent: AgentKind::Codex,
            source_session_id: "s1".into(),
            project_path: None,
            git_repository: None,
            git_head: None,
            included_sections: vec![],
            content_hashes: BTreeMap::new(),
            warnings: vec![],
        };
        filesystem::write_json(&dir.join("manifest.json"), &manifest).unwrap();
    }
    #[test]
    fn detects_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let report = validate(dir.path()).unwrap();
        assert!(!report.valid);
        assert!(report.issues.iter().any(|i| i.code == "missing_file"));
    }
    #[test]
    fn detects_hash_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        minimal(dir.path());
        let mut manifest: PackageManifest =
            serde_json::from_str(&fs::read_to_string(dir.path().join("manifest.json")).unwrap())
                .unwrap();
        manifest
            .content_hashes
            .insert("goal.json".into(), "wrong".into());
        filesystem::write_json(&dir.path().join("manifest.json"), &manifest).unwrap();
        let report = validate(dir.path()).unwrap();
        assert!(report.issues.iter().any(|i| i.code == "hash_mismatch"));
    }
}
