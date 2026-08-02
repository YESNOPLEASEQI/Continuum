use crate::{
    codex_runtime, database,
    error::{AppError, AppResult},
    filesystem,
    models::{DiagnosticPathStatus, DiagnosticsReport},
    security_scanner, settings,
};
use chrono::Utc;
use rusqlite::OptionalExtension;
use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

fn command_version(command: &str, arguments: &[&str]) -> Option<String> {
    codex_runtime::output_with_timeout(command, arguments, Duration::from_secs(5))
        .ok()
        .filter(|output| output.status.success())
        .map(|output| {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            if stdout.is_empty() {
                String::from_utf8_lossy(&output.stderr).trim().to_owned()
            } else {
                stdout
            }
        })
        .filter(|value| !value.is_empty())
}

fn status(path: PathBuf) -> DiagnosticPathStatus {
    let exists = path.exists();
    let readable = if path.is_dir() {
        fs::read_dir(&path).is_ok()
    } else {
        fs::File::open(&path).is_ok()
    };
    let writable = path
        .metadata()
        .map(|metadata| !metadata.permissions().readonly())
        .unwrap_or_else(|_| {
            path.parent()
                .and_then(|parent| parent.metadata().ok())
                .is_some_and(|metadata| !metadata.permissions().readonly())
        });
    DiagnosticPathStatus {
        path: path.to_string_lossy().into_owned(),
        readable,
        writable,
        exists,
    }
}

fn default_session_paths() -> Vec<PathBuf> {
    dirs::home_dir()
        .map(|home| vec![home.join(".codex").join("sessions")])
        .unwrap_or_default()
}

pub fn validate_settings_paths(settings: &crate::models::AppSettings) -> Vec<DiagnosticPathStatus> {
    let mut paths = settings
        .session_paths
        .iter()
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    paths.extend([
        PathBuf::from(&settings.default_working_directory),
        PathBuf::from(&settings.package_output_path),
        PathBuf::from(&settings.backup_directory),
    ]);
    paths
        .into_iter()
        .filter(|path| !path.as_os_str().is_empty())
        .map(status)
        .collect()
}

pub fn collect(db_path: &Path, data_dir: &Path, force_codex: bool) -> AppResult<DiagnosticsReport> {
    let app_settings = settings::load(db_path, data_dir)?;
    let paths = if app_settings.session_paths.is_empty() {
        default_session_paths()
    } else {
        app_settings
            .session_paths
            .iter()
            .map(PathBuf::from)
            .collect()
    };
    let conn = database::connect(db_path)?;
    let recent_scan = conn
        .query_row(
            "SELECT completed_at || ' · ' || status || ' · ' || discovered_count FROM scan_jobs ORDER BY started_at DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()?;
    let recent_continuation = conn
        .query_row(
            "SELECT id || ' · ' || status || ' · ' || updated_at FROM continuations ORDER BY updated_at DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()?;
    let recent_errors = {
        let mut statement = conn.prepare("SELECT area || '/' || code || ': ' || message FROM diagnostics_events WHERE resolved_at IS NULL ORDER BY created_at DESC LIMIT 20")?;
        let first = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        if first.is_empty() {
            let mut statement = conn.prepare("SELECT error_code || ': ' || message FROM session_scan_errors WHERE resolved_at IS NULL ORDER BY occurred_at DESC LIMIT 20")?;
            let values = statement
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            values
        } else {
            first
        }
    };
    drop(conn);
    let log_directory = data_dir.join("logs");
    fs::create_dir_all(&log_directory)?;
    let os_version = command_version("cmd.exe", &["/C", "ver"])
        .unwrap_or_else(|| format!("{} {}", std::env::consts::OS, std::env::consts::ARCH));
    let webview_version = command_version(
        "reg.exe",
        &[
            "query",
            r"HKCU\Software\Microsoft\EdgeUpdate\Clients",
            "/s",
            "/v",
            "pv",
        ],
    )
    .and_then(|output| {
        output
            .lines()
            .find(|line| line.contains("REG_SZ"))
            .and_then(|line| line.split("REG_SZ").nth(1))
            .map(str::trim)
            .map(str::to_owned)
    });
    Ok(DiagnosticsReport {
        continuum_version: env!("CARGO_PKG_VERSION").into(),
        os_version,
        webview_version,
        node_version: command_version("node", &["--version"]),
        rust_version: command_version("rustc", &["--version"]),
        codex: codex_runtime::detect(db_path, data_dir, force_codex)?,
        session_paths: paths.into_iter().map(status).collect(),
        database: database::health(db_path)?,
        watcher_enabled: app_settings.auto_watch,
        watcher_interval_seconds: app_settings.auto_scan_interval_seconds,
        recent_scan,
        recent_continuation,
        recent_errors,
        log_directory: log_directory.to_string_lossy().into_owned(),
        data_directory: data_dir.to_string_lossy().into_owned(),
        backup_count: database::list_backups(db_path)?.len(),
        generated_at: Utc::now().to_rfc3339(),
    })
}

pub fn sanitized_json(report: &DiagnosticsReport) -> AppResult<String> {
    let mut value = serde_json::to_value(report)?;
    security_scanner::redact_diagnostics_value(&mut value);
    Ok(serde_json::to_string_pretty(&value)?)
}

pub fn export(report: &DiagnosticsReport, path: &Path) -> AppResult<String> {
    let extension = path.extension().and_then(|value| value.to_str());
    if extension != Some("json") {
        return Err(AppError::Message("诊断报告必须导出为 .json 文件".into()));
    }
    filesystem::write_atomic(path, sanitized_json(report)?.as_bytes())?;
    Ok(path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{CodexCapabilityReport, DatabaseHealth};

    #[test]
    fn diagnostic_json_redacts_home_and_tokens() {
        let home = dirs::home_dir().unwrap();
        let report = DiagnosticsReport {
            continuum_version: "test".into(),
            os_version: "test".into(),
            webview_version: None,
            node_version: None,
            rust_version: None,
            codex: CodexCapabilityReport {
                capability_schema_version: 2,
                installed: false,
                executable_path: Some(
                    home.join("sk-abcdefghijklmnopqrstuvwxyz")
                        .to_string_lossy()
                        .into_owned(),
                ),
                version: None,
                help_hash: None,
                supports_resume: false,
                supports_fork: false,
                supports_cd: false,
                supports_model: false,
                supports_profile: false,
                supports_sandbox: false,
                supports_approval: false,
                supports_app_server: false,
                session_paths: vec![],
                checked_at: "now".into(),
                error: None,
            },
            session_paths: vec![],
            database: DatabaseHealth {
                path: home
                    .join("continuum.sqlite3")
                    .to_string_lossy()
                    .into_owned(),
                schema_version: database::LATEST_SCHEMA_VERSION,
                integrity: "ok".into(),
                size_bytes: 0,
                orphan_nodes: 0,
                invalid_bindings: 0,
                checked_at: "now".into(),
            },
            watcher_enabled: false,
            watcher_interval_seconds: 5,
            recent_scan: None,
            recent_continuation: None,
            recent_errors: vec!["Authorization: Bearer abcdefghijklmnopqrstuvwxyz".into()],
            log_directory: home.join("logs").to_string_lossy().into_owned(),
            data_directory: home.to_string_lossy().into_owned(),
            backup_count: 0,
            generated_at: "now".into(),
        };
        let output = sanitized_json(&report).unwrap();
        assert!(!output.contains(&home.to_string_lossy().to_string()));
        assert!(!output.contains("abcdefghijklmnopqrstuvwxyz"));
        assert!(output.contains("[REDACTED]"));
    }
}
