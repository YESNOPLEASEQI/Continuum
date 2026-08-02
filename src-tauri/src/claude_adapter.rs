#![allow(dead_code)] // Reserved for the explicit "other Agent" P2 path.

use crate::{
    agent_adapters::AgentAdapter,
    error::{AppError, AppResult},
    models::*,
};
use std::path::{Path, PathBuf};

pub struct ClaudeAdapter;
impl ClaudeAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl AgentAdapter for ClaudeAdapter {
    fn kind(&self) -> AgentKind {
        AgentKind::Claude
    }
    fn detect_installation(&self) -> bool {
        std::process::Command::new("claude")
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
    fn get_version(&self) -> Option<String> {
        std::process::Command::new("claude")
            .arg("--version")
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
    }
    fn get_capabilities(&self) -> AdapterCapabilities {
        AdapterCapabilities {
            native_resume: false,
            native_fork: false,
            fresh_context_launch: false,
            session_watch: false,
            skills_discovery: false,
            mcp_discovery: false,
            status: "framework_only".into(),
        }
    }
    fn default_session_paths(&self) -> Vec<PathBuf> {
        dirs::home_dir()
            .map(|home| vec![home.join(".claude").join("projects")])
            .unwrap_or_default()
    }
    fn scan_sessions(&self, _paths: &[PathBuf]) -> AppResult<Vec<SessionDetail>> {
        Ok(Vec::new())
    }
    fn parse_session(&self, _path: &Path) -> AppResult<SessionDetail> {
        Err(AppError::Message(
            "Claude Code Adapter 只有框架，尚未实现会话解析".into(),
        ))
    }
    fn extract_messages(&self, _raw: &[serde_json::Value]) -> Vec<SessionMessage> {
        Vec::new()
    }
    fn extract_tool_calls(&self, _raw: &[serde_json::Value]) -> Vec<ToolCall> {
        Vec::new()
    }
    fn extract_file_changes(&self, _raw: &[serde_json::Value]) -> Vec<String> {
        Vec::new()
    }
    fn extract_commands(&self, _raw: &[serde_json::Value]) -> Vec<String> {
        Vec::new()
    }
    fn build_resume_prompt(&self, _session: &SessionDetail) -> String {
        "Claude Code Adapter 尚未实现自动续接。".into()
    }
}
