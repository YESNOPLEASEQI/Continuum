mod agent_adapters;
mod claude_adapter;
mod codex_adapter;
mod codex_app_server;
mod codex_runtime;
mod commands;
mod configuration;
mod context_compiler;
mod continuation;
mod database;
mod diagnostics;
mod error;
mod filesystem;
mod git_inspector;
mod logging;
mod models;
mod package_builder;
mod package_validator;
mod profiles;
mod search;
mod security_scanner;
mod session_indexer;
mod session_scanner;
mod settings;
mod unified_project;

use std::path::PathBuf;
use tauri::Manager;

pub struct AppState {
    pub db_path: PathBuf,
    pub data_dir: PathBuf,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    logging::initialize();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .map_err(|error| format!("无法确定应用数据目录：{error}"))?;
            std::fs::create_dir_all(&data_dir)?;
            let db_path = data_dir.join("continuum.sqlite3");
            database::initialize(&db_path).map_err(|error| error.to_string())?;
            let settings =
                database::get_settings(&db_path, &data_dir).map_err(|error| error.to_string())?;
            std::fs::create_dir_all(&settings.package_output_path)?;
            app.manage(AppState { db_path, data_dir });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_dashboard,
            commands::list_sessions,
            commands::get_session,
            commands::scan_sessions,
            commands::detect_codex_capabilities,
            commands::probe_codex_app_server,
            commands::poll_session_changes,
            commands::reindex_session,
            commands::get_settings,
            commands::save_settings,
            commands::prepare_package_draft,
            commands::create_package,
            commands::list_packages,
            commands::get_package,
            commands::validate_package,
            commands::import_package,
            commands::export_package_zip,
            commands::export_package_folder,
            commands::delete_package,
            commands::mark_package_resumed,
            commands::list_projects,
            commands::create_project,
            commands::get_project,
            commands::archive_project,
            commands::restore_project,
            commands::rename_project,
            commands::relocate_project,
            commands::delete_project_record,
            commands::unbind_project_session,
            commands::rebind_project_session,
            commands::suggest_projects_for_session,
            commands::check_database,
            commands::create_database_backup,
            commands::list_database_backups,
            commands::restore_database_backup,
            commands::get_diagnostics,
            commands::copy_diagnostics_report,
            commands::export_diagnostics_report,
            commands::validate_settings_paths,
            commands::bind_sessions_to_project,
            commands::get_unified_timeline,
            commands::add_user_note,
            commands::create_conversation_branch,
            commands::update_conversation_node,
            commands::rename_conversation_branch,
            commands::archive_conversation_branch,
            commands::restore_conversation_branch,
            commands::switch_conversation_branch,
            commands::delete_conversation_branch,
            commands::compare_conversation_branches,
            commands::merge_branch_context_items,
            commands::global_search,
            commands::sync_project_sessions,
            commands::compile_context,
            commands::save_context_snapshot,
            commands::list_context_snapshots,
            commands::diff_context_snapshots,
            commands::set_context_item_override,
            commands::create_continuation,
            commands::launch_continuation,
            commands::list_continuations,
            commands::poll_continuation,
            commands::bind_continuation_session,
            commands::cancel_continuation,
            commands::retry_continuation,
            commands::recover_continuations,
            commands::cleanup_continuation_context,
            commands::launch_source_session,
            commands::scan_configurations,
            commands::bind_configuration,
            commands::list_codex_profiles,
            commands::create_default_codex_profile,
            commands::save_codex_profile,
            commands::duplicate_codex_profile,
            commands::delete_codex_profile,
            commands::set_project_codex_profile,
            commands::set_branch_codex_profile,
            commands::export_codex_profile,
            commands::import_codex_profile
        ])
        .run(tauri::generate_context!())
        .expect("Continuum failed to start");
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::models::AgentKind;
    use std::{
        fs,
        process::Command,
        thread,
        time::{Duration, Instant},
    };

    fn git(repo: &std::path::Path, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(repo)
            .status()
            .expect("git executable");
        assert!(status.success(), "git command failed: {args:?}");
    }

    #[test]
    fn scans_git_builds_zips_and_reimports_a_real_package() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("agentpack.sqlite3");
        let repo = temp.path().join("repo");
        let sessions = temp.path().join("sessions");
        let output = temp.path().join("packages");
        fs::create_dir_all(&repo).unwrap();
        fs::create_dir_all(&sessions).unwrap();
        database::initialize(&db_path).unwrap();

        git(&repo, &["init", "--quiet"]);
        git(
            &repo,
            &["config", "user.email", "agentpack@example.invalid"],
        );
        git(&repo, &["config", "user.name", "AgentPack Test"]);
        fs::write(repo.join("tracked.txt"), "initial\n").unwrap();
        git(&repo, &["add", "tracked.txt"]);
        git(&repo, &["commit", "--quiet", "-m", "initial"]);
        fs::write(repo.join("tracked.txt"), "changed\n").unwrap();
        fs::write(repo.join("untracked.txt"), "new\n").unwrap();

        let jsonl = [
            serde_json::json!({"type":"session_meta","payload":{"id":"integration-session","cwd":repo,"timestamp":"2026-07-30T10:00:00Z"}}),
            serde_json::json!({"type":"response_item","payload":{"role":"user","content":[{"type":"input_text","text":"Finish the integration workflow"}],"timestamp":"2026-07-30T10:01:00Z"}}),
            serde_json::json!({"type":"function_call","payload":{"name":"exec_command","arguments":"{\"cmd\":\"git status --short\"}","output":" M tracked.txt\\n?? untracked.txt"}}),
        ].into_iter().map(|value| serde_json::to_string(&value).unwrap()).collect::<Vec<_>>().join("\n");
        fs::write(sessions.join("integration-session.jsonl"), jsonl).unwrap();

        let mut app_settings = database::default_settings(&db_path, temp.path());
        app_settings.session_paths = vec![sessions.to_string_lossy().into_owned()];
        app_settings.package_output_path = output.to_string_lossy().into_owned();
        let scanned = session_scanner::scan(&db_path, &app_settings).unwrap();
        assert_eq!(scanned.len(), 1);
        let session = database::get_session(&db_path, "integration-session").unwrap();
        assert!(session
            .git_state
            .as_ref()
            .unwrap()
            .modified
            .contains(&"tracked.txt".into()));

        let mut draft = package_builder::prepare_draft(&session);
        draft.title = "Integration handoff".into();
        draft.target_agent = AgentKind::Claude;
        draft.include_untracked = true;
        let built = package_builder::build(&db_path, &app_settings, &draft).unwrap();
        let built_path = std::path::PathBuf::from(&built.package_path);
        assert!(built_path.join("manifest.json").is_file());
        assert!(built_path.with_extension("agentpack.zip").is_file());
        assert!(package_validator::validate(&built_path).unwrap().valid);

        let import_root = temp.path().join("imported");
        app_settings.package_output_path = import_root.to_string_lossy().into_owned();
        let imported = package_builder::import(
            &db_path,
            &app_settings,
            &built_path.with_extension("agentpack.zip"),
        )
        .unwrap();
        assert_eq!(imported.id, built.id);
        assert!(imported.imported);
        assert!(std::path::Path::new(&imported.package_path)
            .join("goal.json")
            .is_file());
    }

    #[test]
    fn fresh_continuation_detects_binds_syncs_and_persists_a_new_codex_session() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("continuum.sqlite3");
        let repo = temp.path().join("repo");
        let sessions = temp.path().join("sessions");
        fs::create_dir_all(&repo).unwrap();
        fs::create_dir_all(&sessions).unwrap();
        database::initialize(&db_path).unwrap();

        let source_records = [
            serde_json::json!({"type":"session_meta","payload":{"id":"source-session","cwd":repo,"timestamp":"2026-07-30T10:00:00Z"}}),
            serde_json::json!({"type":"response_item","payload":{"role":"user","content":[{"type":"input_text","text":"Implement the unified continuation flow"}],"timestamp":"2026-07-30T10:01:00Z"}}),
            serde_json::json!({"type":"response_item","payload":{"role":"assistant","content":[{"type":"output_text","text":"The scanner and project model are ready."}],"timestamp":"2026-07-30T10:02:00Z"}}),
        ];
        let source_jsonl = source_records
            .iter()
            .map(|value| serde_json::to_string(value).unwrap())
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(sessions.join("source-session.jsonl"), source_jsonl).unwrap();

        let mut app_settings = database::default_settings(&db_path, temp.path());
        app_settings.session_paths = vec![sessions.to_string_lossy().into_owned()];
        settings::save(&db_path, &app_settings).unwrap();
        let scanned = session_scanner::scan(&db_path, &app_settings).unwrap();
        assert_eq!(scanned.len(), 1);

        let project = unified_project::create(
            &db_path,
            &crate::models::CreateProjectInput {
                name: "Continuation acceptance".into(),
                project_path: repo.to_string_lossy().into_owned(),
                goal: "Continue work in a clean Codex session".into(),
                constraints: vec!["Do not inherit the old session history".into()],
                default_agent: AgentKind::Codex,
                default_model: "default".into(),
            },
            32_000,
        )
        .unwrap();
        unified_project::bind_sessions(
            &db_path,
            &project.summary.id,
            &["source-session".into()],
            None,
            32_000,
        )
        .unwrap();
        let options = crate::models::ContextCompileOptions {
            project_id: project.summary.id.clone(),
            branch_id: project.summary.current_branch_id.clone(),
            source_node_id: None,
            target_agent: AgentKind::Codex,
            target_model: "default".into(),
            token_budget: 16_000,
            recent_rounds: 8,
            include_tool_logs: true,
            include_git_diff: true,
            include_failed_attempts: true,
            include_skills: true,
            include_mcp: true,
        };
        let compiled = context_compiler::compile(&db_path, &options).unwrap();
        assert!(compiled
            .compiled_text
            .contains("Continue work in a clean Codex session"));

        let record = continuation::create(&db_path, temp.path(), &options, false).unwrap();
        assert_eq!(record.status, "preparing_launch");
        assert!(std::path::Path::new(&record.bootstrap_file).is_file());
        assert!(fs::read_to_string(&record.bootstrap_file)
            .unwrap()
            .contains(&record.marker));
        database::connect(&db_path).unwrap().execute("UPDATE continuations SET status='waiting_detection',started_at='2026-07-31T00:00:00Z' WHERE id=?1",rusqlite::params![record.id]).unwrap();

        let fresh_records = vec![
            serde_json::json!({"type":"session_meta","payload":{"id":"fresh-session","cwd":repo,"timestamp":"2099-07-31T00:00:01Z"}}),
            serde_json::json!({"type":"response_item","payload":{"role":"user","content":[{"type":"input_text","text":format!("{} Read the generated context file first.",record.marker)}],"timestamp":"2099-07-31T00:00:02Z"}}),
        ];
        let fresh_path = sessions.join("fresh-session.jsonl");
        fs::write(
            &fresh_path,
            fresh_records
                .iter()
                .map(|value| serde_json::to_string(value).unwrap())
                .collect::<Vec<_>>()
                .join("\n"),
        )
        .unwrap();

        let detected = continuation::poll(&db_path, temp.path(), &record.id).unwrap();
        assert_eq!(detected.continuation.status, "listening");
        assert_eq!(
            detected.continuation.target_session_id.as_deref(),
            Some("fresh-session")
        );

        let mut updated_records = fresh_records;
        updated_records.push(serde_json::json!({"type":"response_item","payload":{"role":"assistant","content":[{"type":"output_text","text":"Workspace checked; continuing the highest-priority task."}],"timestamp":"2099-07-31T00:00:03Z"}}));
        fs::write(
            &fresh_path,
            updated_records
                .iter()
                .map(|value| serde_json::to_string(value).unwrap())
                .collect::<Vec<_>>()
                .join("\n"),
        )
        .unwrap();
        let synced = continuation::poll(&db_path, temp.path(), &record.id).unwrap();
        assert_eq!(synced.inserted_nodes, 1);
        let timeline = unified_project::timeline(
            &db_path,
            &project.summary.id,
            &project.summary.current_branch_id,
        )
        .unwrap();
        assert!(timeline
            .iter()
            .any(|node| node.content.contains("Workspace checked")));
        assert!(timeline
            .iter()
            .any(|node| node.node_type == "session_switch" && node.content.contains("压缩到约")));

        database::initialize(&db_path).unwrap();
        let persisted = continuation::get(&db_path, &record.id).unwrap();
        assert!(persisted.listening);
        assert_eq!(
            persisted.target_session_id.as_deref(),
            Some("fresh-session")
        );
        let reopened = unified_project::get(&db_path, &project.summary.id, 32_000).unwrap();
        assert!(reopened
            .sessions
            .iter()
            .any(|session| session.id == "fresh-session"
                && session.continuation_id.as_deref() == Some(record.id.as_str())));
    }

    #[test]
    #[ignore = "creates a real local Codex session and waits for its first response"]
    fn real_app_server_fresh_continuation_creates_binds_and_indexes_a_session() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("continuum-real.sqlite3");
        let repo = temp.path().join("workspace");
        let seed_sessions = temp.path().join("seed-sessions");
        fs::create_dir_all(&repo).unwrap();
        fs::create_dir_all(&seed_sessions).unwrap();
        database::initialize(&db_path).unwrap();
        fs::write(
            repo.join("README.md"),
            "# Continuum real acceptance probe\n\nDo not modify this directory.\n",
        )
        .unwrap();

        let seed = [
            serde_json::json!({"type":"session_meta","payload":{"id":"real-acceptance-source","cwd":repo,"timestamp":"2026-08-01T00:00:00Z"}}),
            serde_json::json!({"type":"response_item","payload":{"role":"user","content":[{"type":"input_text","text":"Verify that a clean continuation can read its compiled context and report success without editing files."}],"timestamp":"2026-08-01T00:00:01Z"}}),
        ]
        .into_iter()
        .map(|value| serde_json::to_string(&value).unwrap())
        .collect::<Vec<_>>()
        .join("\n");
        fs::write(seed_sessions.join("source.jsonl"), seed).unwrap();

        let mut app_settings = database::default_settings(&db_path, temp.path());
        let real_session_paths = app_settings.session_paths.clone();
        assert!(
            !real_session_paths.is_empty(),
            "a readable local Codex session directory is required"
        );
        app_settings.session_paths = vec![seed_sessions.to_string_lossy().into_owned()];
        settings::save(&db_path, &app_settings).unwrap();
        assert!(session_scanner::scan(&db_path, &app_settings)
            .unwrap()
            .iter()
            .any(|session| session.id == "real-acceptance-source"));

        app_settings.session_paths = real_session_paths.clone();
        settings::save(&db_path, &app_settings).unwrap();

        let capabilities = codex_runtime::detect(&db_path, temp.path(), true).unwrap();
        assert!(
            capabilities.installed,
            "Codex CLI is required: {capabilities:#?}"
        );
        assert!(
            capabilities.supports_app_server,
            "Codex App Server support is required"
        );
        let project = unified_project::create(
            &db_path,
            &crate::models::CreateProjectInput {
                name: "Real App Server acceptance".into(),
                project_path: repo.to_string_lossy().into_owned(),
                goal: "Read the continuation context, inspect the workspace without modifying it, and reply with REAL_FRESH_ACCEPTANCE_OK.".into(),
                constraints: vec!["Do not edit files and do not run destructive commands.".into()],
                default_agent: AgentKind::Codex,
                default_model: "default".into(),
            },
            16_000,
        )
        .unwrap();
        unified_project::bind_sessions(
            &db_path,
            &project.summary.id,
            &["real-acceptance-source".into()],
            None,
            16_000,
        )
        .unwrap();
        let mut profile = profiles::default_profile(
            Some(project.summary.id.clone()),
            None,
            repo.to_string_lossy().into_owned(),
            &capabilities,
            16_000,
        );
        profile.name = "Real acceptance / no approvals".into();
        profile.approval_mode = "never".into();
        profile.sandbox_mode = "read-only".into();
        let profile = profiles::save(&db_path, profile, &capabilities).unwrap();
        profiles::set_project_default(&db_path, &project.summary.id, &profile.id).unwrap();
        let options = crate::models::ContextCompileOptions {
            project_id: project.summary.id.clone(),
            branch_id: project.summary.current_branch_id.clone(),
            source_node_id: None,
            target_agent: AgentKind::Codex,
            target_model: "default".into(),
            token_budget: 8_000,
            recent_rounds: 8,
            include_tool_logs: true,
            include_git_diff: false,
            include_failed_attempts: true,
            include_skills: false,
            include_mcp: false,
        };
        let continuation = continuation::create(&db_path, temp.path(), &options, true).unwrap();
        assert_eq!(continuation.status, "listening");
        let target_session_id = continuation
            .target_session_id
            .clone()
            .expect("App Server must return a thread id");

        let deadline = Instant::now() + Duration::from_secs(180);
        let target_path = loop {
            let found = real_session_paths.iter().find_map(|root| {
                walkdir::WalkDir::new(root)
                    .follow_links(false)
                    .into_iter()
                    .filter_map(Result::ok)
                    .find(|entry| {
                        entry.file_type().is_file()
                            && entry
                                .file_name()
                                .to_string_lossy()
                                .contains(&target_session_id)
                    })
                    .map(|entry| entry.into_path())
            });
            if let Some(path) = found {
                break path;
            }
            assert!(
                Instant::now() < deadline,
                "new session JSONL was not created"
            );
            thread::sleep(Duration::from_secs(1));
        };
        app_settings.session_paths = vec![target_path
            .parent()
            .expect("session file parent")
            .to_string_lossy()
            .into_owned()];
        settings::save(&db_path, &app_settings).unwrap();
        let mut saw_target_session = false;
        let mut saw_assistant_message = false;
        while Instant::now() < deadline {
            continuation::poll(&db_path, temp.path(), &continuation.id).unwrap();
            if let Ok(detail) = database::get_session(&db_path, &target_session_id) {
                saw_target_session = true;
                saw_assistant_message = detail.messages.iter().any(|message| {
                    matches!(message.role, crate::models::MessageRole::Assistant)
                        && !message.content.trim().is_empty()
                });
                if saw_assistant_message {
                    break;
                }
            }
            thread::sleep(Duration::from_secs(1));
        }
        assert!(saw_target_session, "new session JSONL was not indexed");
        assert!(
            saw_assistant_message,
            "new session produced no assistant message"
        );
        database::initialize(&db_path).unwrap();
        let persisted = continuation::get(&db_path, &continuation.id).unwrap();
        assert_eq!(
            persisted.target_session_id.as_deref(),
            Some(target_session_id.as_str())
        );
        assert!(persisted.listening);
    }
}
