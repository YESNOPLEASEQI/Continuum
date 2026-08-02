use crate::{
    database,
    error::{AppError, AppResult},
    models::AppSettings,
};
use std::path::Path;

pub fn validate(settings: &AppSettings) -> AppResult<()> {
    if settings.package_output_path.trim().is_empty() {
        return Err(AppError::Message("任务包输出目录不能为空".into()));
    }
    if !matches!(settings.theme.as_str(), "dark" | "system") {
        return Err(AppError::Message("不支持的 UI 主题".into()));
    }
    if !matches!(
        settings.log_level.as_str(),
        "error" | "warn" | "info" | "debug"
    ) {
        return Err(AppError::Message("不支持的日志级别".into()));
    }
    if settings.default_context_budget < 1_000 {
        return Err(AppError::Message("默认上下文预算至少为 1000 Token".into()));
    }
    if !matches!(
        settings.compression_strategy.as_str(),
        "balanced" | "conservative" | "aggressive"
    ) {
        return Err(AppError::Message("不支持的上下文压缩策略".into()));
    }
    if settings.codex_command.trim().is_empty() {
        return Err(AppError::Message("Codex 启动命令不能为空".into()));
    }
    if !(1..=500).contains(&settings.recent_message_limit) {
        return Err(AppError::Message("最近消息数量必须在 1 到 500 之间".into()));
    }
    if !(2..=3600).contains(&settings.auto_scan_interval_seconds) {
        return Err(AppError::Message(
            "自动扫描间隔必须在 2 到 3600 秒之间".into(),
        ));
    }
    if !(100..=5_000_000).contains(&settings.tool_output_max_length) {
        return Err(AppError::Message(
            "工具输出最大长度必须在 100 到 5,000,000 之间".into(),
        ));
    }
    if !(0.1..=2.0).contains(&settings.health_warning_ratio)
        || !(settings.health_warning_ratio..=2.0).contains(&settings.health_critical_ratio)
    {
        return Err(AppError::Message(
            "健康阈值必须递增且位于 0.1 到 2.0 之间".into(),
        ));
    }
    for path in &settings.session_paths {
        if !Path::new(path).is_dir() || std::fs::read_dir(path).is_err() {
            return Err(AppError::Message(format!(
                "Codex sessions 目录不可读：{path}"
            )));
        }
    }
    if !settings.default_working_directory.trim().is_empty()
        && !Path::new(&settings.default_working_directory).is_dir()
    {
        return Err(AppError::Message("默认项目目录不存在".into()));
    }
    Ok(())
}
pub fn load(db_path: &Path, data_dir: &Path) -> AppResult<AppSettings> {
    database::get_settings(db_path, data_dir)
}
pub fn save(db_path: &Path, settings: &AppSettings) -> AppResult<AppSettings> {
    validate(settings)?;
    std::fs::create_dir_all(&settings.package_output_path)?;
    if !settings.backup_directory.trim().is_empty() {
        std::fs::create_dir_all(&settings.backup_directory)?;
    }
    database::save_settings(db_path, settings)?;
    Ok(settings.clone())
}
