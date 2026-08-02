use crate::agent_adapters::AgentAdapter;
use crate::{
    codex_adapter::CodexAdapter,
    database,
    error::{AppError, AppResult},
    filesystem, git_inspector,
    models::*,
};
use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

fn agent(value: &str) -> Option<AgentKind> {
    match value {
        "codex" => Some(AgentKind::Codex),
        "claude" => Some(AgentKind::Claude),
        "gemini" => Some(AgentKind::Gemini),
        "opencode" => Some(AgentKind::Opencode),
        "cursor" => Some(AgentKind::Cursor),
        "copilot" => Some(AgentKind::Copilot),
        _ => None,
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
fn now() -> String {
    Utc::now().to_rfc3339()
}
fn parse_metadata(value: String) -> BTreeMap<String, serde_json::Value> {
    serde_json::from_str(&value).unwrap_or_default()
}

pub fn create(
    db_path: &Path,
    input: &CreateProjectInput,
    context_budget: usize,
) -> AppResult<UnifiedProjectDetail> {
    if input.name.trim().is_empty() || input.goal.trim().is_empty() {
        return Err(AppError::Message("项目名称和总体目标不能为空".into()));
    }
    let path = Path::new(&input.project_path);
    if !path.is_dir() {
        return Err(AppError::Message("项目路径不存在或不是目录".into()));
    }
    let id = uuid::Uuid::new_v4().to_string();
    let branch_id = uuid::Uuid::new_v4().to_string();
    let created = now();
    let git = git_inspector::inspect(path);
    let mut conn = database::connect(db_path)?;
    let normalized_path = filesystem::normalize_path_key(path);
    let duplicate: Option<String> = conn
        .query_row(
            "SELECT id FROM projects WHERE normalized_path=?1 AND deleted_at IS NULL LIMIT 1",
            params![normalized_path],
            |row| row.get(0),
        )
        .optional()?;
    if duplicate.is_some() {
        return Err(AppError::Message(
            "该工作目录已经存在于 Continuum 项目中".into(),
        ));
    }
    let tx = conn.transaction()?;
    tx.execute("INSERT INTO projects(id,name,project_path,normalized_path,display_path,git_repository,goal,constraints_json,default_agent,default_model,current_branch_id,default_branch_id,current_task,archived,created_at,updated_at,last_opened_at) VALUES(?1,?2,?3,?4,?3,?5,?6,?7,?8,?9,?10,?10,'',0,?11,?11,?11)",params![id,input.name.trim(),input.project_path,normalized_path,git.repository_path,input.goal.trim(),serde_json::to_string(&input.constraints)?,agent_name(&input.default_agent),input.default_model,branch_id,created])?;
    tx.execute("INSERT INTO conversation_branches(id,project_id,name,status,created_at,updated_at) VALUES(?1,?2,'main','active',?3,?3)",params![branch_id,id,created])?;
    let root_id = uuid::Uuid::new_v4().to_string();
    tx.execute("INSERT INTO conversation_nodes(id,project_id,branch_id,node_type,content,created_at,importance,status,metadata_json) VALUES(?1,?2,?3,'summary',?4,?5,100,'active',?6)",params![root_id,id,branch_id,format!("统一项目已创建：{}",input.goal.trim()),created,json!({"kind":"project_root"}).to_string()])?;
    for constraint in &input.constraints {
        let node_id = uuid::Uuid::new_v4().to_string();
        tx.execute("INSERT INTO conversation_nodes(id,project_id,parent_node_id,branch_id,node_type,content,created_at,importance,status,metadata_json) VALUES(?1,?2,?3,?4,'constraint',?5,?6,100,'active','{}')",params![node_id,id,root_id,branch_id,constraint,created])?;
    }
    tx.commit()?;
    get(db_path, &id, context_budget)
}

pub fn list(db_path: &Path, context_budget: usize) -> AppResult<Vec<UnifiedProjectSummary>> {
    let conn = database::connect(db_path)?;
    let mut stmt = conn.prepare("SELECT id FROM projects ORDER BY archived,updated_at DESC")?;
    let ids = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    drop(stmt);
    drop(conn);
    ids.into_iter()
        .map(|id| get_summary(db_path, &id, context_budget))
        .collect()
}

pub fn get_summary(
    db_path: &Path,
    id: &str,
    context_budget: usize,
) -> AppResult<UnifiedProjectSummary> {
    let conn = database::connect(db_path)?;
    let row=conn.query_row("SELECT p.id,p.name,p.project_path,p.git_repository,p.goal,p.current_task,p.current_branch_id,b.name,p.default_agent,p.default_model,p.updated_at,p.archived,(SELECT COUNT(*) FROM project_bindings pb WHERE pb.project_id=p.id AND pb.binding_type='source_session') FROM projects p JOIN conversation_branches b ON b.id=p.current_branch_id WHERE p.id=?1",params![id],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?,row.get::<_,Option<String>>(3)?,row.get::<_,String>(4)?,row.get::<_,String>(5)?,row.get::<_,String>(6)?,row.get::<_,String>(7)?,row.get::<_,String>(8)?,row.get::<_,String>(9)?,row.get::<_,String>(10)?,row.get::<_,i64>(11)?,row.get::<_,i64>(12)?))).optional()?.ok_or_else(||AppError::Message("找不到统一项目".into()))?;
    let nodes = timeline(db_path, &row.0, &row.6)?;
    let health =
        crate::context_compiler::calculate_health(db_path, &row.0, &nodes, context_budget)?;
    let path_exists = Path::new(&row.2).is_dir();
    Ok(UnifiedProjectSummary {
        id: row.0,
        name: row.1,
        project_path: row.2,
        git_repository: row.3,
        goal: row.4,
        current_task: row.5,
        current_branch_id: row.6,
        current_branch_name: row.7,
        default_agent: agent(&row.8).unwrap_or(AgentKind::Codex),
        default_model: row.9,
        session_count: row.12 as usize,
        updated_at: row.10,
        archived: row.11 != 0,
        path_exists,
        health,
    })
}

pub fn get(db_path: &Path, id: &str, context_budget: usize) -> AppResult<UnifiedProjectDetail> {
    database::connect(db_path)?.execute(
        "UPDATE projects SET last_opened_at=?1 WHERE id=?2",
        params![now(), id],
    )?;
    let summary = get_summary(db_path, id, context_budget)?;
    let conn = database::connect(db_path)?;
    let constraints: String = conn.query_row(
        "SELECT constraints_json FROM projects WHERE id=?1",
        params![id],
        |row| row.get(0),
    )?;
    let branches = list_branches(db_path, id)?;
    let mut stmt=conn.prepare("SELECT ss.id,ss.agent_type,ss.title,ss.source_path,pb.branch_id,(SELECT COUNT(*) FROM source_messages sm WHERE sm.source_session_id=ss.id),pb.created_at,json_extract(pb.metadata_json,'$.continuationId') FROM project_bindings pb JOIN source_sessions ss ON ss.id=pb.binding_id WHERE pb.project_id=?1 AND pb.binding_type='source_session' ORDER BY pb.created_at")?;
    let sessions = stmt
        .query_map(params![id], |row| {
            Ok(BoundSourceSession {
                id: row.get(0)?,
                agent: agent(&row.get::<_, String>(1)?).unwrap_or(AgentKind::Codex),
                title: row.get(2)?,
                source_path: row.get(3)?,
                branch_id: row
                    .get::<_, Option<String>>(4)?
                    .unwrap_or_else(|| summary.current_branch_id.clone()),
                message_count: row.get::<_, i64>(5)? as usize,
                last_synced_at: row.get(6)?,
                continuation_id: row.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let nodes = timeline(db_path, id, &summary.current_branch_id)?;
    let active_files = nodes
        .iter()
        .filter(|node| node.node_type == "file_change" && node.status == "active")
        .map(|node| node.content.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let decisions = nodes
        .iter()
        .filter(|node| node.node_type == "decision")
        .cloned()
        .collect();
    let todos = nodes
        .iter()
        .filter(|node| node.node_type == "todo" && node.status == "active")
        .cloned()
        .collect();
    let git_state = Some(git_inspector::inspect(Path::new(&summary.project_path)));
    Ok(UnifiedProjectDetail {
        summary,
        constraints: serde_json::from_str(&constraints).unwrap_or_default(),
        branches,
        sessions,
        active_files,
        decisions,
        todos,
        git_state,
    })
}

pub fn list_branches(db_path: &Path, project_id: &str) -> AppResult<Vec<ConversationBranch>> {
    let conn = database::connect(db_path)?;
    let mut stmt=conn.prepare("SELECT id,project_id,name,parent_branch_id,fork_node_id,status,created_at,updated_at,(SELECT COUNT(*) FROM conversation_nodes n WHERE n.branch_id=b.id) FROM conversation_branches b WHERE project_id=?1 ORDER BY created_at")?;
    let rows = stmt
        .query_map(params![project_id], |row| {
            Ok(ConversationBranch {
                id: row.get(0)?,
                project_id: row.get(1)?,
                name: row.get(2)?,
                parent_branch_id: row.get(3)?,
                fork_node_id: row.get(4)?,
                status: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
                node_count: row.get::<_, i64>(8)? as usize,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn timeline(
    db_path: &Path,
    project_id: &str,
    branch_id: &str,
) -> AppResult<Vec<ConversationNode>> {
    let conn = database::connect(db_path)?;
    let mut stmt=conn.prepare("SELECT id,project_id,parent_node_id,branch_id,source_agent,source_session_id,node_type,content,created_at,importance,status,metadata_json FROM conversation_nodes WHERE project_id=?1 AND branch_id=?2 ORDER BY created_at,id")?;
    let rows = stmt
        .query_map(params![project_id, branch_id], |row| {
            let source: Option<String> = row.get(4)?;
            Ok(ConversationNode {
                id: row.get(0)?,
                project_id: row.get(1)?,
                parent_node_id: row.get(2)?,
                branch_id: row.get(3)?,
                source_agent: source.as_deref().and_then(agent),
                source_session_id: row.get(5)?,
                node_type: row.get(6)?,
                content: row.get(7)?,
                created_at: row.get(8)?,
                importance: row.get(9)?,
                status: row.get(10)?,
                metadata: parse_metadata(row.get(11)?),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn last_node_id(
    conn: &rusqlite::Connection,
    project_id: &str,
    branch_id: &str,
) -> AppResult<Option<String>> {
    Ok(conn.query_row("SELECT id FROM conversation_nodes WHERE project_id=?1 AND branch_id=?2 ORDER BY created_at DESC,rowid DESC LIMIT 1",params![project_id,branch_id],|row|row.get(0)).optional()?)
}

fn validate_session_binding(
    db_path: &Path,
    project_id: &str,
    detail: &SessionDetail,
    allow_existing_project: bool,
) -> AppResult<()> {
    let conn = database::connect(db_path)?;
    let project_path: String = conn.query_row(
        "SELECT normalized_path FROM projects WHERE id=?1 AND deleted_at IS NULL",
        params![project_id],
        |row| row.get(0),
    )?;
    let session_path = detail
        .summary
        .working_directory
        .as_deref()
        .ok_or_else(|| AppError::Message("来源会话没有可校验的工作目录".into()))?;
    let normalized_session = filesystem::normalize_path_key(Path::new(session_path));
    if normalized_session != project_path {
        return Err(AppError::Message(format!(
            "会话工作目录与项目冲突：会话为 {session_path}，项目为 {project_path}"
        )));
    }
    let existing: Option<String> = conn
        .query_row(
            "SELECT project_id FROM project_bindings WHERE binding_type='source_session' AND binding_id=?1 LIMIT 1",
            params![detail.summary.id],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(existing_project) = existing {
        if existing_project != project_id && !allow_existing_project {
            return Err(AppError::Message(format!(
                "该会话已经绑定到项目 {existing_project}；请使用重新绑定操作"
            )));
        }
    }
    Ok(())
}

pub fn bind_sessions(
    db_path: &Path,
    project_id: &str,
    session_ids: &[String],
    branch_id: Option<&str>,
    context_budget: usize,
) -> AppResult<UnifiedProjectDetail> {
    let project = get_summary(db_path, project_id, context_budget)?;
    let branch = branch_id.unwrap_or(&project.current_branch_id).to_owned();
    let mut details = session_ids
        .iter()
        .map(|id| database::get_session(db_path, id))
        .collect::<AppResult<Vec<_>>>()?;
    details.sort_by(|a, b| a.summary.created_at.cmp(&b.summary.created_at));
    for detail in &details {
        validate_session_binding(db_path, project_id, detail, false)?;
    }
    for detail in details {
        bind_detail(db_path, project_id, &branch, &detail, None)?;
    }
    get(db_path, project_id, context_budget)
}

pub fn bind_detail(
    db_path: &Path,
    project_id: &str,
    branch_id: &str,
    detail: &SessionDetail,
    continuation_id: Option<&str>,
) -> AppResult<usize> {
    let mut conn = database::connect(db_path)?;
    let already:i64=conn.query_row("SELECT COUNT(*) FROM project_bindings WHERE project_id=?1 AND binding_type='source_session' AND binding_id=?2",params![project_id,detail.summary.id],|row|row.get(0))?;
    if already > 0 {
        return Ok(0);
    }
    let tx = conn.transaction()?;
    tx.execute("INSERT INTO source_sessions(id,agent_type,title,source_path,working_directory,created_at,updated_at,detail_json) VALUES(?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(id) DO UPDATE SET title=excluded.title,updated_at=excluded.updated_at,detail_json=excluded.detail_json",params![detail.summary.id,agent_name(&detail.summary.agent),detail.summary.title,detail.summary.source_path,detail.summary.working_directory,detail.summary.created_at,detail.summary.updated_at,serde_json::to_string(detail)?])?;
    tx.execute("INSERT INTO project_bindings(project_id,binding_type,binding_id,branch_id,created_at,metadata_json) VALUES(?1,'source_session',?2,?3,?4,?5)",params![project_id,detail.summary.id,branch_id,now(),json!({"continuationId":continuation_id}).to_string()])?;
    tx.execute("UPDATE source_sessions SET bound_project_id=?1,bound_branch_id=?2,status='bound' WHERE id=?3",params![project_id,branch_id,detail.summary.id])?;
    tx.execute("UPDATE conversation_branches SET current_session_id=?1,updated_at=?2 WHERE id=?3 AND project_id=?4",params![detail.summary.id,now(),branch_id,project_id])?;
    let mut parent = last_node_id(&tx, project_id, branch_id)?;
    let switch_id = format!("switch:{project_id}:{branch_id}:{}", detail.summary.id);
    tx.execute("INSERT OR IGNORE INTO conversation_nodes(id,project_id,parent_node_id,branch_id,source_agent,source_session_id,node_type,content,created_at,importance,status,metadata_json) VALUES(?1,?2,?3,?4,?5,?6,'session_switch',?7,?8,80,'active',?9)",params![switch_id,project_id,parent,branch_id,agent_name(&detail.summary.agent),detail.summary.id,format!("切换到 {} 会话：{}",agent_name(&detail.summary.agent),detail.summary.title),detail.summary.created_at,json!({"sourcePath":detail.summary.source_path}).to_string()])?;
    parent = Some(switch_id);
    let mut inserted = 0;
    for (index, message) in detail.messages.iter().enumerate() {
        let source_id = format!("{}:{}", detail.summary.id, message.id);
        tx.execute("INSERT OR IGNORE INTO source_messages(id,source_session_id,role,content,created_at,raw_index) VALUES(?1,?2,?3,?4,?5,?6)",params![source_id,detail.summary.id,format!("{:?}",message.role).to_lowercase(),message.content,message.timestamp,index as i64])?;
        let node_id = format!("node:{project_id}:{branch_id}:{source_id}");
        let timestamp = message
            .timestamp
            .clone()
            .unwrap_or_else(|| detail.summary.created_at.clone());
        let metadata=json!({"role":format!("{:?}",message.role).to_lowercase(),"sourceMessageId":message.id,"rawIndex":index}).to_string();
        let changed=tx.execute("INSERT OR IGNORE INTO conversation_nodes(id,project_id,parent_node_id,branch_id,source_agent,source_session_id,node_type,content,created_at,importance,status,metadata_json) VALUES(?1,?2,?3,?4,?5,?6,'message',?7,?8,50,'active',?9)",params![node_id,project_id,parent,branch_id,agent_name(&detail.summary.agent),detail.summary.id,message.content,timestamp,metadata])?;
        if changed > 0 {
            parent = Some(node_id);
            inserted += 1;
        }
    }
    for (index, tool) in detail.tool_calls.iter().enumerate() {
        let source_id = format!("{}:{}", detail.summary.id, tool.id);
        tx.execute("INSERT OR IGNORE INTO tool_calls_v2(id,source_session_id,name,arguments,status,output,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![source_id,detail.summary.id,tool.name,tool.arguments,format!("{:?}",tool.status).to_lowercase(),tool.output,tool.timestamp])?;
        let node_id = format!("tool:{project_id}:{branch_id}:{source_id}");
        let content = format!(
            "{}\n参数：{}\n结果：{}",
            tool.name,
            tool.arguments,
            tool.output.as_deref().unwrap_or("未记录")
        );
        let changed=tx.execute("INSERT OR IGNORE INTO conversation_nodes(id,project_id,parent_node_id,branch_id,source_agent,source_session_id,node_type,content,created_at,importance,status,metadata_json) VALUES(?1,?2,?3,?4,?5,?6,'tool_call',?7,?8,30,?9,?10)",params![node_id,project_id,parent,branch_id,agent_name(&detail.summary.agent),detail.summary.id,content,tool.timestamp.clone().unwrap_or_else(||detail.summary.updated_at.clone()),if matches!(tool.status,ToolStatus::Failed){"active"}else{"completed"},json!({"toolName":tool.name,"index":index}).to_string()])?;
        if changed > 0 {
            parent = Some(node_id);
            inserted += 1;
        }
    }
    for file in &detail.changed_files {
        let change_id = format!("file:{}:{}", detail.summary.id, file);
        tx.execute("INSERT OR IGNORE INTO file_changes(id,source_session_id,path,change_type,created_at) VALUES(?1,?2,?3,'modified',?4)",params![change_id,detail.summary.id,file,detail.summary.updated_at])?;
        let node_id = format!("node:{project_id}:{branch_id}:{change_id}");
        if tx.execute("INSERT OR IGNORE INTO conversation_nodes(id,project_id,parent_node_id,branch_id,source_agent,source_session_id,node_type,content,created_at,importance,status,metadata_json) VALUES(?1,?2,?3,?4,?5,?6,'file_change',?7,?8,65,'active','{}')",params![node_id,project_id,parent,branch_id,agent_name(&detail.summary.agent),detail.summary.id,file,detail.summary.updated_at])?>0{parent=Some(node_id);inserted+=1;}
    }
    for (index, error) in detail.failed_steps.iter().enumerate() {
        let node_id = format!(
            "error:{project_id}:{branch_id}:{}:{index}",
            detail.summary.id
        );
        if tx.execute("INSERT OR IGNORE INTO conversation_nodes(id,project_id,parent_node_id,branch_id,source_agent,source_session_id,node_type,content,created_at,importance,status,metadata_json) VALUES(?1,?2,?3,?4,?5,?6,'error',?7,?8,85,'active','{}')",params![node_id,project_id,parent,branch_id,agent_name(&detail.summary.agent),detail.summary.id,error,detail.summary.updated_at])?>0{parent=Some(node_id);inserted+=1;}
    }
    tx.execute("UPDATE projects SET updated_at=?1,current_task=CASE WHEN current_task='' THEN ?2 ELSE current_task END WHERE id=?3",params![now(),detail.goal_summary,project_id])?;
    tx.commit()?;
    Ok(inserted)
}

pub fn add_note(
    db_path: &Path,
    project_id: &str,
    branch_id: &str,
    content: &str,
    parent_node_id: Option<&str>,
) -> AppResult<ConversationNode> {
    if content.trim().is_empty() {
        return Err(AppError::Message("备注不能为空".into()));
    }
    let conn = database::connect(db_path)?;
    let parent = parent_node_id
        .map(str::to_owned)
        .or(last_node_id(&conn, project_id, branch_id)?);
    let node = ConversationNode {
        id: uuid::Uuid::new_v4().to_string(),
        project_id: project_id.into(),
        parent_node_id: parent,
        branch_id: branch_id.into(),
        source_agent: None,
        source_session_id: None,
        node_type: "user_note".into(),
        content: content.trim().into(),
        created_at: now(),
        importance: 70,
        status: "active".into(),
        metadata: BTreeMap::new(),
    };
    conn.execute("INSERT INTO conversation_nodes(id,project_id,parent_node_id,branch_id,node_type,content,created_at,importance,status,metadata_json) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,'{}')",params![node.id,node.project_id,node.parent_node_id,node.branch_id,node.node_type,node.content,node.created_at,node.importance,node.status])?;
    conn.execute(
        "UPDATE projects SET updated_at=?1,current_task=?2 WHERE id=?3",
        params![now(), node.content, project_id],
    )?;
    Ok(node)
}

pub fn create_branch(
    db_path: &Path,
    project_id: &str,
    from_node_id: &str,
    name: &str,
) -> AppResult<ConversationBranch> {
    if name.trim().is_empty() {
        return Err(AppError::Message("分支名称不能为空".into()));
    }
    let conn = database::connect(db_path)?;
    let parent_branch: String = conn
        .query_row(
            "SELECT branch_id FROM conversation_nodes WHERE id=?1 AND project_id=?2",
            params![from_node_id, project_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| AppError::Message("找不到分支起点".into()))?;
    let id = uuid::Uuid::new_v4().to_string();
    let created = now();
    conn.execute("INSERT INTO conversation_branches(id,project_id,name,parent_branch_id,fork_node_id,status,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,'active',?6,?6)",params![id,project_id,name.trim(),parent_branch,from_node_id,created])?;
    let node_id = uuid::Uuid::new_v4().to_string();
    conn.execute("INSERT INTO conversation_nodes(id,project_id,parent_node_id,branch_id,node_type,content,created_at,importance,status,metadata_json) VALUES(?1,?2,?3,?4,'branch_point',?5,?6,80,'active',?7)",params![node_id,project_id,from_node_id,id,format!("从节点创建分支：{}",name.trim()),created,json!({"parentBranchId":parent_branch}).to_string()])?;
    conn.execute(
        "UPDATE projects SET current_branch_id=?1,updated_at=?2 WHERE id=?3",
        params![id, created, project_id],
    )?;
    Ok(ConversationBranch {
        id,
        project_id: project_id.into(),
        name: name.trim().into(),
        parent_branch_id: Some(parent_branch),
        fork_node_id: Some(from_node_id.into()),
        status: "active".into(),
        created_at: created.clone(),
        updated_at: created,
        node_count: 1,
    })
}

pub fn rename_branch(db_path: &Path, branch_id: &str, name: &str) -> AppResult<()> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::Message("分支名称不能为空".into()));
    }
    let conn = database::connect(db_path)?;
    let project_id: String = conn
        .query_row(
            "SELECT project_id FROM conversation_branches WHERE id=?1",
            params![branch_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| AppError::Message("找不到分支".into()))?;
    let duplicate: i64 = conn.query_row(
        "SELECT COUNT(*) FROM conversation_branches WHERE project_id=?1 AND lower(name)=lower(?2) AND id<>?3",
        params![project_id,name,branch_id],
        |row| row.get(0),
    )?;
    if duplicate > 0 {
        return Err(AppError::Message("同一项目中已存在同名分支".into()));
    }
    conn.execute(
        "UPDATE conversation_branches SET name=?1,updated_at=?2 WHERE id=?3",
        params![name, now(), branch_id],
    )?;
    Ok(())
}

pub fn archive_branch(db_path: &Path, branch_id: &str) -> AppResult<()> {
    let mut conn = database::connect(db_path)?;
    let transaction = conn.transaction()?;
    let (project_id, default_branch_id): (String, String) = transaction
        .query_row(
            "SELECT b.project_id,p.default_branch_id FROM conversation_branches b JOIN projects p ON p.id=b.project_id WHERE b.id=?1",
            params![branch_id],
            |row| Ok((row.get(0)?,row.get(1)?)),
        )
        .optional()?
        .ok_or_else(|| AppError::Message("找不到分支".into()))?;
    if branch_id == default_branch_id {
        return Err(AppError::Message("默认分支不能归档".into()));
    }
    let timestamp = now();
    transaction.execute(
        "UPDATE conversation_branches SET status='archived',archived_at=?1,updated_at=?1 WHERE id=?2",
        params![timestamp,branch_id],
    )?;
    transaction.execute(
        "UPDATE projects SET current_branch_id=CASE WHEN current_branch_id=?1 THEN default_branch_id ELSE current_branch_id END,updated_at=?2 WHERE id=?3",
        params![branch_id,timestamp,project_id],
    )?;
    transaction.commit()?;
    Ok(())
}

pub fn restore_branch(db_path: &Path, branch_id: &str) -> AppResult<()> {
    let changed = database::connect(db_path)?.execute(
        "UPDATE conversation_branches SET status='active',archived_at=NULL,updated_at=?1 WHERE id=?2",
        params![now(),branch_id],
    )?;
    if changed == 0 {
        return Err(AppError::Message("找不到分支".into()));
    }
    Ok(())
}

pub fn switch_branch(db_path: &Path, project_id: &str, branch_id: &str) -> AppResult<()> {
    let conn = database::connect(db_path)?;
    let exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM conversation_branches WHERE id=?1 AND project_id=?2 AND status='active'",
        params![branch_id,project_id],
        |row| row.get(0),
    )?;
    if exists == 0 {
        return Err(AppError::Message(
            "目标分支不存在、已归档或不属于该项目".into(),
        ));
    }
    conn.execute(
        "UPDATE projects SET current_branch_id=?1,updated_at=?2,last_opened_at=?2 WHERE id=?3",
        params![branch_id, now(), project_id],
    )?;
    Ok(())
}

pub fn delete_branch(db_path: &Path, branch_id: &str) -> AppResult<()> {
    let mut conn = database::connect(db_path)?;
    let transaction = conn.transaction()?;
    let (project_id, default_branch_id): (String, String) = transaction
        .query_row(
            "SELECT b.project_id,p.default_branch_id FROM conversation_branches b JOIN projects p ON p.id=b.project_id WHERE b.id=?1",
            params![branch_id],
            |row| Ok((row.get(0)?,row.get(1)?)),
        )
        .optional()?
        .ok_or_else(|| AppError::Message("找不到分支".into()))?;
    if branch_id == default_branch_id {
        return Err(AppError::Message("默认分支不能删除".into()));
    }
    let child_count: i64 = transaction.query_row(
        "SELECT COUNT(*) FROM conversation_branches WHERE parent_branch_id=?1",
        params![branch_id],
        |row| row.get(0),
    )?;
    if child_count > 0 {
        return Err(AppError::Message(
            "该分支仍有子分支；请先归档或处理子分支".into(),
        ));
    }
    let binding_count: i64 = transaction.query_row(
        "SELECT COUNT(*) FROM project_bindings WHERE project_id=?1 AND branch_id=?2",
        params![project_id, branch_id],
        |row| row.get(0),
    )?;
    let continuation_count: i64 = transaction.query_row(
        "SELECT COUNT(*) FROM continuations WHERE project_id=?1 AND branch_id=?2",
        params![project_id, branch_id],
        |row| row.get(0),
    )?;
    if binding_count > 0 || continuation_count > 0 {
        return Err(AppError::Message(
            "该分支仍绑定会话、配置或 Continuation；为避免来源链断裂，不能删除".into(),
        ));
    }
    transaction.execute(
        "DELETE FROM project_bindings WHERE project_id=?1 AND branch_id=?2",
        params![project_id, branch_id],
    )?;
    transaction.execute(
        "DELETE FROM conversation_branches WHERE id=?1",
        params![branch_id],
    )?;
    transaction.execute(
        "UPDATE projects SET current_branch_id=CASE WHEN current_branch_id=?1 THEN default_branch_id ELSE current_branch_id END,updated_at=?2 WHERE id=?3",
        params![branch_id,now(),project_id],
    )?;
    transaction.commit()?;
    Ok(())
}

fn branch_category_values(
    conn: &rusqlite::Connection,
    branch_id: &str,
) -> AppResult<BTreeMap<String, BTreeSet<String>>> {
    let mut result = BTreeMap::<String, BTreeSet<String>>::new();
    let mut statement = conn.prepare("SELECT node_type,content FROM conversation_nodes WHERE branch_id=?1 AND status NOT IN ('incorrect','excluded') ORDER BY created_at,id")?;
    for value in statement.query_map(params![branch_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })? {
        let (kind, content) = value?;
        let category = match kind.as_str() {
            "decision" => "decisions",
            "constraint" => "constraints",
            "todo" => "current_tasks",
            "file_change" => "active_files",
            "error" => "errors",
            "test_result" => "test_results",
            _ => continue,
        };
        result.entry(category.into()).or_default().insert(content);
    }
    Ok(result)
}

pub fn compare_branches(
    db_path: &Path,
    source_branch_id: &str,
    target_branch_id: &str,
) -> AppResult<BranchComparison> {
    let conn = database::connect(db_path)?;
    let project_count: i64 = conn.query_row(
        "SELECT COUNT(DISTINCT project_id) FROM conversation_branches WHERE id IN (?1,?2)",
        params![source_branch_id, target_branch_id],
        |row| row.get(0),
    )?;
    if project_count != 1 {
        return Err(AppError::Message("只能比较同一项目中的两个有效分支".into()));
    }
    let source = branch_category_values(&conn, source_branch_id)?;
    let target = branch_category_values(&conn, target_branch_id)?;
    let categories = source
        .keys()
        .chain(target.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut source_only = BTreeMap::new();
    let mut target_only = BTreeMap::new();
    for category in categories {
        let left = source.get(&category).cloned().unwrap_or_default();
        let right = target.get(&category).cloned().unwrap_or_default();
        source_only.insert(category.clone(), left.difference(&right).cloned().collect());
        target_only.insert(category, right.difference(&left).cloned().collect());
    }
    Ok(BranchComparison {
        source_branch_id: source_branch_id.into(),
        target_branch_id: target_branch_id.into(),
        source_only,
        target_only,
    })
}

pub fn merge_branch_nodes(
    db_path: &Path,
    source_branch_id: &str,
    target_branch_id: &str,
    node_ids: &[String],
) -> AppResult<ConversationNode> {
    if source_branch_id == target_branch_id || node_ids.is_empty() {
        return Err(AppError::Message("请选择来自另一分支的上下文项".into()));
    }
    let conn = database::connect(db_path)?;
    let project_id: String = conn.query_row(
        "SELECT project_id FROM conversation_branches WHERE id=?1 AND status='active'",
        params![target_branch_id],
        |row| row.get(0),
    )?;
    let source_project: String = conn.query_row(
        "SELECT project_id FROM conversation_branches WHERE id=?1",
        params![source_branch_id],
        |row| row.get(0),
    )?;
    if source_project != project_id {
        return Err(AppError::Message("不能跨项目合并上下文项".into()));
    }
    let mut merged = Vec::new();
    for node_id in node_ids {
        let node: Option<(String, String)> = conn
            .query_row(
                "SELECT node_type,content FROM conversation_nodes WHERE id=?1 AND branch_id=?2 AND status NOT IN ('incorrect','excluded')",
                params![node_id,source_branch_id],
                |row| Ok((row.get(0)?,row.get(1)?)),
            )
            .optional()?;
        if let Some((node_type, content)) = node {
            merged.push(json!({"sourceNodeId":node_id,"nodeType":node_type,"content":content}));
        }
    }
    if merged.is_empty() {
        return Err(AppError::Message("所选节点均无可合并内容".into()));
    }
    insert_event(
        db_path,
        &project_id,
        target_branch_id,
        "summary",
        &format!(
            "从分支 {source_branch_id} 合并 {} 个上下文项（未修改代码）",
            merged.len()
        ),
        json!({"mergeType":"context_items","sourceBranchId":source_branch_id,"items":merged}),
    )
}

pub fn update_node(
    db_path: &Path,
    node_id: &str,
    status: &str,
    importance: i32,
) -> AppResult<ConversationNode> {
    if !matches!(
        status,
        "active" | "completed" | "stale" | "incorrect" | "excluded"
    ) {
        return Err(AppError::Message("不支持的节点状态".into()));
    }
    let conn = database::connect(db_path)?;
    conn.execute(
        "UPDATE conversation_nodes SET status=?1,importance=?2 WHERE id=?3",
        params![status, importance.clamp(0, 100), node_id],
    )?;
    get_node(&conn, node_id)
}
fn get_node(conn: &rusqlite::Connection, node_id: &str) -> AppResult<ConversationNode> {
    let tuple=conn.query_row("SELECT id,project_id,parent_node_id,branch_id,source_agent,source_session_id,node_type,content,created_at,importance,status,metadata_json FROM conversation_nodes WHERE id=?1",params![node_id],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,Option<String>>(2)?,row.get::<_,String>(3)?,row.get::<_,Option<String>>(4)?,row.get::<_,Option<String>>(5)?,row.get::<_,String>(6)?,row.get::<_,String>(7)?,row.get::<_,String>(8)?,row.get::<_,i32>(9)?,row.get::<_,String>(10)?,row.get::<_,String>(11)?))).optional()?.ok_or_else(||AppError::Message("找不到节点".into()))?;
    Ok(ConversationNode {
        id: tuple.0,
        project_id: tuple.1,
        parent_node_id: tuple.2,
        branch_id: tuple.3,
        source_agent: tuple.4.as_deref().and_then(agent),
        source_session_id: tuple.5,
        node_type: tuple.6,
        content: tuple.7,
        created_at: tuple.8,
        importance: tuple.9,
        status: tuple.10,
        metadata: parse_metadata(tuple.11),
    })
}

pub fn archive(db_path: &Path, id: &str) -> AppResult<()> {
    database::connect(db_path)?.execute(
        "UPDATE projects SET archived=1,updated_at=?1 WHERE id=?2",
        params![now(), id],
    )?;
    Ok(())
}

pub fn restore_project(db_path: &Path, id: &str) -> AppResult<()> {
    let changed = database::connect(db_path)?.execute(
        "UPDATE projects SET archived=0,updated_at=?1,deleted_at=NULL WHERE id=?2",
        params![now(), id],
    )?;
    if changed == 0 {
        return Err(AppError::Message("找不到统一项目".into()));
    }
    Ok(())
}

pub fn rename_project(db_path: &Path, id: &str, name: &str) -> AppResult<()> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::Message("项目名称不能为空".into()));
    }
    let changed = database::connect(db_path)?.execute(
        "UPDATE projects SET name=?1,updated_at=?2 WHERE id=?3 AND deleted_at IS NULL",
        params![name, now(), id],
    )?;
    if changed == 0 {
        return Err(AppError::Message("找不到统一项目".into()));
    }
    Ok(())
}

pub fn relocate_project(db_path: &Path, id: &str, project_path: &str) -> AppResult<()> {
    let path = Path::new(project_path);
    if !path.is_dir() {
        return Err(AppError::Message("新的项目目录不存在或不可读".into()));
    }
    let normalized = filesystem::normalize_path_key(path);
    let conn = database::connect(db_path)?;
    let duplicate: Option<String> = conn
        .query_row(
            "SELECT id FROM projects WHERE normalized_path=?1 AND id<>?2 AND deleted_at IS NULL LIMIT 1",
            params![normalized, id],
            |row| row.get(0),
        )
        .optional()?;
    if duplicate.is_some() {
        return Err(AppError::Message(
            "新的目录已绑定到另一个 Continuum 项目".into(),
        ));
    }
    let git = git_inspector::inspect(path);
    let changed = conn.execute(
        "UPDATE projects SET project_path=?1,display_path=?1,normalized_path=?2,git_repository=?3,updated_at=?4,last_opened_at=?4 WHERE id=?5 AND deleted_at IS NULL",
        params![project_path, normalized, git.repository_path, now(), id],
    )?;
    if changed == 0 {
        return Err(AppError::Message("找不到统一项目".into()));
    }
    Ok(())
}

pub fn delete_project_record(db_path: &Path, id: &str) -> AppResult<()> {
    let mut conn = database::connect(db_path)?;
    let transaction = conn.transaction()?;
    let exists: i64 = transaction.query_row(
        "SELECT COUNT(*) FROM projects WHERE id=?1",
        params![id],
        |row| row.get(0),
    )?;
    if exists == 0 {
        return Err(AppError::Message("找不到统一项目".into()));
    }
    transaction.execute(
        "UPDATE source_sessions SET bound_project_id=NULL,bound_branch_id=NULL,status='indexed' WHERE bound_project_id=?1",
        params![id],
    )?;
    transaction.execute("DELETE FROM projects WHERE id=?1", params![id])?;
    transaction.commit()?;
    Ok(())
}

pub fn unbind_session(db_path: &Path, project_id: &str, session_id: &str) -> AppResult<()> {
    let mut conn = database::connect(db_path)?;
    let transaction = conn.transaction()?;
    let branch_id: Option<String> = transaction
        .query_row(
            "SELECT branch_id FROM project_bindings WHERE project_id=?1 AND binding_type='source_session' AND binding_id=?2",
            params![project_id, session_id],
            |row| row.get(0),
        )
        .optional()?;
    let Some(branch_id) = branch_id else {
        return Err(AppError::Message("该会话未绑定到指定项目".into()));
    };
    transaction.execute(
        "DELETE FROM conversation_nodes WHERE project_id=?1 AND branch_id=?2 AND source_session_id=?3",
        params![project_id, branch_id, session_id],
    )?;
    transaction.execute(
        "DELETE FROM project_bindings WHERE project_id=?1 AND binding_type='source_session' AND binding_id=?2",
        params![project_id, session_id],
    )?;
    transaction.execute(
        "UPDATE source_sessions SET bound_project_id=NULL,bound_branch_id=NULL,status='indexed' WHERE id=?1",
        params![session_id],
    )?;
    transaction.execute(
        "UPDATE conversation_branches SET current_session_id=NULL,updated_at=?1 WHERE id=?2 AND current_session_id=?3",
        params![now(), branch_id, session_id],
    )?;
    transaction.execute(
        "UPDATE projects SET updated_at=?1 WHERE id=?2",
        params![now(), project_id],
    )?;
    transaction.commit()?;
    Ok(())
}

pub fn rebind_session(
    db_path: &Path,
    session_id: &str,
    target_project_id: &str,
    target_branch_id: &str,
    context_budget: usize,
) -> AppResult<UnifiedProjectDetail> {
    let detail = database::get_session(db_path, session_id)?;
    validate_session_binding(db_path, target_project_id, &detail, true)?;
    let current: Option<String> = database::connect(db_path)?
        .query_row(
            "SELECT project_id FROM project_bindings WHERE binding_type='source_session' AND binding_id=?1 LIMIT 1",
            params![session_id],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(project_id) = current {
        unbind_session(db_path, &project_id, session_id)?;
    }
    bind_detail(db_path, target_project_id, target_branch_id, &detail, None)?;
    get(db_path, target_project_id, context_budget)
}

pub fn suggested_projects(
    db_path: &Path,
    session_id: &str,
    context_budget: usize,
) -> AppResult<Vec<UnifiedProjectSummary>> {
    let detail = database::get_session(db_path, session_id)?;
    let Some(cwd) = detail.summary.working_directory.as_deref() else {
        return Ok(vec![]);
    };
    let normalized = filesystem::normalize_path_key(Path::new(cwd));
    let conn = database::connect(db_path)?;
    let ids = {
        let mut statement = conn.prepare("SELECT id FROM projects WHERE normalized_path=?1 AND archived=0 AND deleted_at IS NULL ORDER BY last_opened_at DESC")?;
        let rows = statement
            .query_map(params![normalized], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };
    ids.into_iter()
        .map(|id| get_summary(db_path, &id, context_budget))
        .collect()
}

pub fn insert_event(
    db_path: &Path,
    project_id: &str,
    branch_id: &str,
    node_type: &str,
    content: &str,
    metadata: serde_json::Value,
) -> AppResult<ConversationNode> {
    let conn = database::connect(db_path)?;
    let parent = last_node_id(&conn, project_id, branch_id)?;
    let node = ConversationNode {
        id: uuid::Uuid::new_v4().to_string(),
        project_id: project_id.into(),
        parent_node_id: parent,
        branch_id: branch_id.into(),
        source_agent: None,
        source_session_id: None,
        node_type: node_type.into(),
        content: content.into(),
        created_at: now(),
        importance: 85,
        status: "active".into(),
        metadata: serde_json::from_value(metadata).unwrap_or_default(),
    };
    conn.execute("INSERT INTO conversation_nodes(id,project_id,parent_node_id,branch_id,node_type,content,created_at,importance,status,metadata_json) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",params![node.id,node.project_id,node.parent_node_id,node.branch_id,node.node_type,node.content,node.created_at,node.importance,node.status,serde_json::to_string(&node.metadata)?])?;
    Ok(node)
}

pub fn sync(db_path: &Path, project_id: &str) -> AppResult<usize> {
    let conn = database::connect(db_path)?;
    let mut stmt=conn.prepare("SELECT ss.source_path,pb.branch_id,json_extract(pb.metadata_json,'$.continuationId') FROM project_bindings pb JOIN source_sessions ss ON ss.id=pb.binding_id WHERE pb.project_id=?1 AND pb.binding_type='source_session'")?;
    let sources = stmt
        .query_map(params![project_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(stmt);
    drop(conn);
    let adapter = CodexAdapter::new();
    let mut inserted = 0;
    for (path, branch, continuation) in sources {
        if let Ok(detail) = adapter.parse_session(Path::new(&path)) {
            let conn = database::connect(db_path)?;
            conn.execute(
                "UPDATE source_sessions SET updated_at=?1,detail_json=?2 WHERE id=?3",
                params![
                    detail.summary.updated_at,
                    serde_json::to_string(&detail)?,
                    detail.summary.id
                ],
            )?;
            drop(conn);
            inserted += append_new_nodes(
                db_path,
                project_id,
                &branch,
                &detail,
                continuation.as_deref(),
            )?;
        }
    }
    Ok(inserted)
}

pub fn sync_indexed_session(db_path: &Path, detail: &SessionDetail) -> AppResult<usize> {
    let conn = database::connect(db_path)?;
    let bindings = {
        let mut statement = conn.prepare("SELECT project_id,branch_id,json_extract(metadata_json,'$.continuationId') FROM project_bindings WHERE binding_type='source_session' AND binding_id=?1")?;
        let rows = statement
            .query_map(params![detail.summary.id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };
    let mut inserted = 0;
    for (project_id, branch_id, continuation_id) in bindings {
        inserted += append_new_nodes(
            db_path,
            &project_id,
            &branch_id,
            detail,
            continuation_id.as_deref(),
        )?;
    }
    Ok(inserted)
}

pub(crate) fn append_new_nodes(
    db_path: &Path,
    project_id: &str,
    branch_id: &str,
    detail: &SessionDetail,
    _continuation: Option<&str>,
) -> AppResult<usize> {
    let conn = database::connect(db_path)?;
    let mut parent = last_node_id(&conn, project_id, branch_id)?;
    let mut inserted = 0;
    for (index, message) in detail.messages.iter().enumerate() {
        let source_id = format!("{}:{}", detail.summary.id, message.id);
        let node_id = format!("node:{project_id}:{branch_id}:{source_id}");
        let exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM conversation_nodes WHERE id=?1",
            params![node_id],
            |row| row.get(0),
        )?;
        if exists > 0 {
            continue;
        }
        conn.execute("INSERT OR IGNORE INTO source_messages(id,source_session_id,role,content,created_at,raw_index) VALUES(?1,?2,?3,?4,?5,?6)",params![source_id,detail.summary.id,format!("{:?}",message.role).to_lowercase(),message.content,message.timestamp,index as i64])?;
        conn.execute("INSERT INTO conversation_nodes(id,project_id,parent_node_id,branch_id,source_agent,source_session_id,node_type,content,created_at,importance,status,metadata_json) VALUES(?1,?2,?3,?4,'codex',?5,'message',?6,?7,50,'active',?8)",params![node_id,project_id,parent,branch_id,detail.summary.id,message.content,message.timestamp.clone().unwrap_or_else(now),json!({"role":format!("{:?}",message.role).to_lowercase(),"rawIndex":index}).to_string()])?;
        parent = Some(node_id);
        inserted += 1;
    }
    for (index, tool) in detail.tool_calls.iter().enumerate() {
        let source_id = format!("{}:{}", detail.summary.id, tool.id);
        let node_id = format!("tool:{project_id}:{branch_id}:{source_id}");
        let exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM conversation_nodes WHERE id=?1",
            params![node_id],
            |row| row.get(0),
        )?;
        if exists > 0 {
            continue;
        }
        conn.execute("INSERT OR IGNORE INTO tool_calls_v2(id,source_session_id,name,arguments,status,output,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![source_id,detail.summary.id,tool.name,tool.arguments,format!("{:?}",tool.status).to_lowercase(),tool.output,tool.timestamp])?;
        let content = format!(
            "{}\n参数：{}\n结果：{}",
            tool.name,
            tool.arguments,
            tool.output.as_deref().unwrap_or("未记录")
        );
        conn.execute("INSERT INTO conversation_nodes(id,project_id,parent_node_id,branch_id,source_agent,source_session_id,source_message_id,node_type,content,created_at,importance,status,metadata_json,imported_at) VALUES(?1,?2,?3,?4,'codex',?5,?6,'tool_call',?7,?8,30,?9,?10,?11)",params![node_id,project_id,parent,branch_id,detail.summary.id,tool.id,content,tool.timestamp.clone().unwrap_or_else(now),if matches!(tool.status,ToolStatus::Failed){"active"}else{"completed"},json!({"toolName":tool.name,"index":index,"collapsed":true}).to_string(),now()])?;
        parent = Some(node_id);
        inserted += 1;
    }
    for file in &detail.changed_files {
        let source_id = format!("{}:{file}", detail.summary.id);
        let node_id = format!("file:{project_id}:{branch_id}:{source_id}");
        if conn.query_row(
            "SELECT COUNT(*) FROM conversation_nodes WHERE id=?1",
            params![node_id],
            |row| row.get::<_, i64>(0),
        )? > 0
        {
            continue;
        }
        conn.execute("INSERT OR IGNORE INTO file_changes(id,source_session_id,path,change_type,created_at) VALUES(?1,?2,?3,'modified',?4)",params![source_id,detail.summary.id,file,detail.summary.updated_at])?;
        conn.execute("INSERT INTO conversation_nodes(id,project_id,parent_node_id,branch_id,source_agent,source_session_id,source_message_id,node_type,content,created_at,importance,status,metadata_json,imported_at) VALUES(?1,?2,?3,?4,'codex',?5,?6,'file_change',?7,?8,65,'active','{}',?9)",params![node_id,project_id,parent,branch_id,detail.summary.id,source_id,file,detail.summary.updated_at,now()])?;
        parent = Some(node_id);
        inserted += 1;
    }
    for (index, error) in detail.failed_steps.iter().enumerate() {
        let source_id = format!("error:{}:{index}", detail.summary.id);
        let node_id = format!("error:{project_id}:{branch_id}:{source_id}");
        if conn.query_row(
            "SELECT COUNT(*) FROM conversation_nodes WHERE id=?1",
            params![node_id],
            |row| row.get::<_, i64>(0),
        )? > 0
        {
            continue;
        }
        conn.execute("INSERT INTO conversation_nodes(id,project_id,parent_node_id,branch_id,source_agent,source_session_id,source_message_id,node_type,content,created_at,importance,status,metadata_json,imported_at) VALUES(?1,?2,?3,?4,'codex',?5,?6,'error',?7,?8,85,'active','{}',?9)",params![node_id,project_id,parent,branch_id,detail.summary.id,source_id,error,detail.summary.updated_at,now()])?;
        parent = Some(node_id);
        inserted += 1;
    }
    if inserted > 0 {
        conn.execute(
            "UPDATE projects SET updated_at=?1 WHERE id=?2",
            params![now(), project_id],
        )?;
    }
    Ok(inserted)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn project_graph_preserves_session_sources() {
        let temp = tempfile::tempdir().unwrap();
        let db = temp.path().join("db.sqlite");
        database::initialize(&db).unwrap();
        let repo = temp.path().join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        let project = create(
            &db,
            &CreateProjectInput {
                name: "Demo".into(),
                project_path: repo.to_string_lossy().into_owned(),
                goal: "Keep context".into(),
                constraints: vec!["Never delete data".into()],
                default_agent: AgentKind::Codex,
                default_model: "default".into(),
            },
            32000,
        )
        .unwrap();
        let detail = SessionDetail {
            summary: SessionSummary {
                id: "s1".into(),
                title: "Source".into(),
                agent: AgentKind::Codex,
                created_at: now(),
                updated_at: now(),
                working_directory: Some(repo.to_string_lossy().into_owned()),
                git_repository: None,
                message_count: 1,
                tool_call_count: 0,
                has_file_changes: false,
                can_package: true,
                source_path: repo.join("s.jsonl").to_string_lossy().into_owned(),
                parse_warning: None,
            },
            goal_summary: "Continue work".into(),
            messages: vec![SessionMessage {
                id: "m1".into(),
                role: MessageRole::User,
                content: "Continue work".into(),
                timestamp: Some(now()),
            }],
            tool_calls: vec![],
            commands: vec![],
            changed_files: vec![],
            failed_steps: vec![],
            git_state: None,
            raw_data: vec![],
        };
        database::upsert_session(&db, &detail).unwrap();
        bind_sessions(&db, &project.summary.id, &["s1".into()], None, 32000).unwrap();
        let nodes = timeline(&db, &project.summary.id, &project.summary.current_branch_id).unwrap();
        assert!(nodes.iter().any(|node| node.node_type == "session_switch"
            && node.source_session_id.as_deref() == Some("s1")));
        assert!(nodes
            .iter()
            .any(|node| node.content == "Continue work"
                && node.source_agent == Some(AgentKind::Codex)));
    }
}
