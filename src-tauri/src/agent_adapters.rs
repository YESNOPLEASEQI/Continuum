#![allow(dead_code)] // Cross-agent P2 contract; Codex is the only active P0 adapter.

use crate::{error::AppResult, models::*};
use std::path::{Path, PathBuf};

pub trait AgentAdapter {
    fn kind(&self) -> AgentKind;
    fn detect_installation(&self) -> bool;
    fn get_version(&self) -> Option<String> {
        None
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
    fn default_session_paths(&self) -> Vec<PathBuf>;
    fn scan_sessions(&self, paths: &[PathBuf]) -> AppResult<Vec<SessionDetail>>;
    fn parse_session(&self, path: &Path) -> AppResult<SessionDetail>;
    fn watch_session(&self, path: &Path) -> AppResult<SessionDetail> {
        self.parse_session(path)
    }
    fn create_session(
        &self,
        _working_directory: &Path,
        _bootstrap_context: &str,
    ) -> AppResult<Option<u32>> {
        Err(crate::error::AppError::Message(
            "该 Adapter 尚未实现 createSession".into(),
        ))
    }
    fn resume_native_session(&self, _session_id: &str) -> AppResult<Option<u32>> {
        Err(crate::error::AppError::Message(
            "该 Adapter 尚未实现 resumeNativeSession".into(),
        ))
    }
    fn launch_with_context(
        &self,
        _working_directory: &Path,
        _context_file: &Path,
        _marker: &str,
    ) -> AppResult<Option<u32>> {
        Err(crate::error::AppError::Message(
            "该 Adapter 尚未实现 launchWithContext".into(),
        ))
    }
    fn extract_messages(&self, raw: &[serde_json::Value]) -> Vec<SessionMessage>;
    fn extract_tool_calls(&self, raw: &[serde_json::Value]) -> Vec<ToolCall>;
    fn extract_file_changes(&self, raw: &[serde_json::Value]) -> Vec<String>;
    fn extract_commands(&self, raw: &[serde_json::Value]) -> Vec<String>;
    fn build_resume_prompt(&self, session: &SessionDetail) -> String;
    fn build_bootstrap_context(&self, session: &SessionDetail) -> String {
        self.build_resume_prompt(session)
    }
    fn get_skills(&self) -> Vec<PathBuf> {
        Vec::new()
    }
    fn get_mcp_servers(&self) -> Vec<String> {
        Vec::new()
    }
    fn get_custom_instructions(&self) -> Vec<PathBuf> {
        Vec::new()
    }
}
