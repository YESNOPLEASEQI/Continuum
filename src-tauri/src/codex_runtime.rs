use crate::{
    database,
    error::{AppError, AppResult},
    models::{AppSettings, CodexCapabilityReport},
    settings,
};
use rusqlite::{params, OptionalExtension};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

fn command_parts(command: &str) -> AppResult<Vec<String>> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    for character in command.trim().chars() {
        match character {
            '"' | '\'' if quote == Some(character) => quote = None,
            '"' | '\'' if quote.is_none() => quote = Some(character),
            value if value.is_whitespace() && quote.is_none() => {
                if !current.is_empty() {
                    parts.push(std::mem::take(&mut current));
                }
            }
            value => current.push(value),
        }
    }
    if quote.is_some() {
        return Err(AppError::Message("Codex 命令包含未闭合引号".into()));
    }
    if !current.is_empty() {
        parts.push(current);
    }
    if parts.is_empty() {
        return Err(AppError::Message("Codex 可执行文件未配置".into()));
    }
    Ok(parts)
}

#[cfg(windows)]
fn where_candidates(executable: &str) -> Vec<PathBuf> {
    let Ok(output) = Command::new("where.exe").arg(executable).output() else {
        return vec![];
    };
    if !output.status.success() {
        return vec![];
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .collect()
}

#[cfg(windows)]
fn preferred_windows_candidate(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates
        .iter()
        .find(|path| path.extension().and_then(|value| value.to_str()) == Some("cmd"))
        .or_else(|| {
            candidates
                .iter()
                .find(|path| path.extension().and_then(|value| value.to_str()) == Some("exe"))
        })
        .or_else(|| {
            candidates
                .iter()
                .find(|path| path.extension().and_then(|value| value.to_str()) == Some("ps1"))
        })
        .cloned()
}

#[cfg(windows)]
fn resolve_windows_executable(executable: &str) -> PathBuf {
    let explicit = PathBuf::from(executable);
    if explicit.is_file() {
        return explicit;
    }
    let mut candidates = where_candidates(executable);
    if let Ok(app_data) = std::env::var("APPDATA") {
        let npm = PathBuf::from(app_data).join("npm");
        for name in ["codex.exe", "codex.cmd", "codex.ps1"] {
            let candidate = npm.join(name);
            if candidate.is_file() && !candidates.contains(&candidate) {
                candidates.push(candidate);
            }
        }
    }
    // `where codex` can include the packaged Codex desktop binary under
    // WindowsApps. That path is visible but may reject direct CreateProcess
    // calls (os error 5). Prefer the user-installed npm shim when available.
    preferred_windows_candidate(&candidates).unwrap_or(explicit)
}

pub fn resolve_command_path(command: &str) -> AppResult<String> {
    let parts = command_parts(command)?;
    let executable = &parts[0];
    #[cfg(windows)]
    let resolved = resolve_windows_executable(executable);
    #[cfg(not(windows))]
    let resolved = PathBuf::from(executable);
    Ok(resolved.to_string_lossy().into_owned())
}

pub fn command(command: &str) -> AppResult<Command> {
    let mut parts = command_parts(command)?;
    let executable = parts.remove(0);
    #[cfg(windows)]
    {
        let resolved = resolve_windows_executable(&executable);
        let lower = resolved.to_string_lossy().to_ascii_lowercase();
        let mut process = if lower.ends_with(".ps1") {
            let mut value = Command::new("powershell.exe");
            value
                .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
                .arg(&resolved);
            value
        } else if lower.ends_with(".cmd") || lower.ends_with(".bat") {
            let mut value = Command::new("cmd.exe");
            value.args(["/D", "/C"]).arg(&resolved);
            value
        } else {
            Command::new(&resolved)
        };
        process.args(parts);
        Ok(process)
    }
    #[cfg(not(windows))]
    {
        let mut process = Command::new(executable);
        process.args(parts);
        Ok(process)
    }
}

pub fn output_with_timeout(
    command_line: &str,
    args: &[&str],
    timeout: Duration,
) -> AppResult<Output> {
    let mut process = command(command_line)?;
    process
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = process
        .spawn()
        .map_err(|error| AppError::Message(format!("无法启动 Codex 能力检测：{error}")))?;
    let started = Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            return child.wait_with_output().map_err(AppError::Io);
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(AppError::Message(format!(
                "Codex 能力检测超过 {} 秒",
                timeout.as_secs()
            )));
        }
        thread::sleep(Duration::from_millis(30));
    }
}

fn output_text(output: &Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stdout.trim().is_empty() {
        stderr.trim().to_owned()
    } else if stderr.trim().is_empty() {
        stdout.trim().to_owned()
    } else {
        format!("{}\n{}", stdout.trim(), stderr.trim())
    }
}

pub fn parse_help(
    executable_path: Option<String>,
    version: Option<String>,
    help: &str,
    session_paths: Vec<String>,
    error: Option<String>,
) -> CodexCapabilityReport {
    let command_line = |name: &str| {
        help.lines().any(|line| {
            let trimmed = line.trim_start();
            trimmed.starts_with(name)
                && trimmed
                    .chars()
                    .nth(name.len())
                    .is_some_and(char::is_whitespace)
        })
    };
    let mut hasher = Sha256::new();
    hasher.update(help.as_bytes());
    CodexCapabilityReport {
        capability_schema_version: 2,
        installed: executable_path.is_some() && version.is_some() && error.is_none(),
        executable_path,
        version,
        help_hash: (!help.is_empty()).then(|| hex::encode(hasher.finalize())),
        supports_resume: command_line("resume"),
        supports_fork: command_line("fork"),
        supports_cd: help.contains("-C, --cd <DIR>"),
        supports_model: help.contains("-m, --model <MODEL>"),
        supports_profile: help.contains("-p, --profile <CONFIG_PROFILE>"),
        supports_sandbox: help.contains("-s, --sandbox <SANDBOX_MODE>"),
        supports_approval: help.contains("-a, --ask-for-approval <APPROVAL_POLICY>"),
        supports_app_server: command_line("app-server"),
        session_paths,
        checked_at: chrono::Utc::now().to_rfc3339(),
        error,
    }
}

fn configured_command(settings: &AppSettings) -> String {
    settings
        .agent_install_paths
        .get("codex")
        .filter(|value| !value.trim().is_empty())
        .cloned()
        .unwrap_or_else(|| settings.codex_command.clone())
}

fn readable_session_paths(settings: &AppSettings) -> (Vec<String>, Vec<String>) {
    let paths = if settings.session_paths.is_empty() {
        dirs::home_dir()
            .map(|home| {
                vec![home
                    .join(".codex")
                    .join("sessions")
                    .to_string_lossy()
                    .into_owned()]
            })
            .unwrap_or_default()
    } else {
        settings.session_paths.clone()
    };
    let mut readable = Vec::new();
    let mut invalid = Vec::new();
    for value in paths {
        let path = Path::new(&value);
        if path.is_dir() && fs::read_dir(path).is_ok() {
            readable.push(value);
        } else {
            invalid.push(value);
        }
    }
    (readable, invalid)
}

pub fn detect(db_path: &Path, data_dir: &Path, force: bool) -> AppResult<CodexCapabilityReport> {
    let app_settings = settings::load(db_path, data_dir)?;
    detect_with_settings(db_path, &app_settings, force)
}

pub fn detect_with_settings(
    db_path: &Path,
    settings: &AppSettings,
    force: bool,
) -> AppResult<CodexCapabilityReport> {
    let configured = configured_command(settings);
    let resolved = resolve_command_path(&configured)?;
    let (session_paths, invalid_paths) = readable_session_paths(settings);
    let version_output = output_with_timeout(&configured, &["--version"], Duration::from_secs(10));
    let version = match &version_output {
        Ok(output) if output.status.success() => Some(output_text(output)),
        Ok(output) => {
            let message = output_text(output);
            return cache_report(
                db_path,
                parse_help(
                    Some(resolved),
                    None,
                    "",
                    session_paths,
                    Some(format!("Codex --version 失败：{message}")),
                ),
            );
        }
        Err(error) => {
            return cache_report(
                db_path,
                parse_help(
                    Some(resolved),
                    None,
                    "",
                    session_paths,
                    Some(error.to_string()),
                ),
            );
        }
    };

    if !force {
        let cached: Option<String> = database::connect(db_path)?
            .query_row(
                "SELECT capabilities_json FROM agent_capabilities WHERE agent_type='codex'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(value) = cached {
            if let Ok(mut report) = serde_json::from_str::<CodexCapabilityReport>(&value) {
                if report.capability_schema_version == 2
                    && report.executable_path.as_deref() == Some(resolved.as_str())
                    && report.version == version
                {
                    report.session_paths = session_paths;
                    report.checked_at = chrono::Utc::now().to_rfc3339();
                    report.error = (!invalid_paths.is_empty())
                        .then(|| format!("以下 sessions 目录不可读：{}", invalid_paths.join("；")));
                    report.installed = report.error.is_none();
                    return cache_report(db_path, report);
                }
            }
        }
    }

    let help_output = output_with_timeout(&configured, &["--help"], Duration::from_secs(10))?;
    let help = output_text(&help_output);
    let error = if !help_output.status.success() {
        Some(format!("Codex --help 失败：{help}"))
    } else if !invalid_paths.is_empty() {
        Some(format!(
            "以下 sessions 目录不可读：{}",
            invalid_paths.join("；")
        ))
    } else {
        None
    };
    cache_report(
        db_path,
        parse_help(Some(resolved), version, &help, session_paths, error),
    )
}

fn cache_report(db_path: &Path, report: CodexCapabilityReport) -> AppResult<CodexCapabilityReport> {
    let mut conn = database::connect(db_path)?;
    let transaction = conn.transaction()?;
    transaction.execute(
        "INSERT INTO agent_capabilities(agent_type,capabilities_json,updated_at) VALUES('codex',?1,?2) ON CONFLICT(agent_type) DO UPDATE SET capabilities_json=excluded.capabilities_json,updated_at=excluded.updated_at",
        params![serde_json::to_string(&report)?, report.checked_at],
    )?;
    if let Some(path) = &report.executable_path {
        transaction.execute(
            "INSERT INTO agent_installations(id,agent_type,path,detected_at) VALUES('codex','codex',?1,?2) ON CONFLICT(id) DO UPDATE SET path=excluded.path,detected_at=excluded.detected_at",
            params![path, report.checked_at],
        )?;
    }
    transaction.commit()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_capabilities_present_in_realistic_help() {
        let report = parse_help(
            Some("codex".into()),
            Some("codex-cli 1.0.0".into()),
            "Commands:\n  resume  Resume session\nOptions:\n  -C, --cd <DIR>\n  -m, --model <MODEL>",
            vec!["sessions".into()],
            None,
        );
        assert!(report.installed);
        assert!(report.supports_resume);
        assert!(report.supports_cd);
        assert!(report.supports_model);
        assert!(!report.supports_fork);
        assert!(!report.supports_profile);
    }

    #[cfg(windows)]
    #[test]
    fn windows_prefers_npm_cmd_over_packaged_desktop_exe() {
        let candidates = vec![
            PathBuf::from(r"C:\Program Files\WindowsApps\OpenAI.Codex\codex.exe"),
            PathBuf::from(r"C:\Users\test\AppData\Roaming\npm\codex.cmd"),
        ];
        assert_eq!(
            preferred_windows_candidate(&candidates),
            Some(candidates[1].clone())
        );
    }

    #[cfg(windows)]
    #[test]
    fn detects_version_and_upgrade_from_custom_windows_command() {
        let temporary = tempfile::tempdir().unwrap();
        let db_path = temporary.path().join("continuum.sqlite3");
        database::initialize(&db_path).unwrap();
        let sessions = temporary.path().join("sessions");
        fs::create_dir_all(&sessions).unwrap();
        let executable = temporary.path().join("codex.cmd");
        fs::write(
            &executable,
            "@echo off\r\nif \"%1\"==\"--version\" (echo codex-cli 1.0.0& exit /b 0)\r\nif \"%1\"==\"--help\" (echo   resume  Resume session& echo   -C, --cd ^<DIR^>& exit /b 0)\r\nexit /b 1\r\n",
        )
        .unwrap();
        let settings = AppSettings {
            session_paths: vec![sessions.to_string_lossy().into_owned()],
            codex_command: executable.to_string_lossy().into_owned(),
            ..AppSettings::default()
        };
        let report = detect_with_settings(&db_path, &settings, false).unwrap();
        assert_eq!(report.version.as_deref(), Some("codex-cli 1.0.0"));
        assert!(report.supports_resume);
        assert!(!report.supports_fork);
    }
}
