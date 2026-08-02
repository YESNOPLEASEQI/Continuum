use crate::{
    database,
    error::{AppError, AppResult},
    git_inspector,
    models::*,
    unified_project,
};
use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

pub trait ContextCompressionProvider {
    fn compile(
        &self,
        db_path: &Path,
        options: &ContextCompileOptions,
    ) -> AppResult<CompiledContext>;
}
pub struct RuleBasedProvider;

fn estimate(value: &str) -> usize {
    value.chars().count().div_ceil(4)
}
fn normalized(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}
fn is_constraint(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "必须", "不得", "不要", "禁止", "约束", "must ", "must not", "never ", "do not",
    ]
    .iter()
    .any(|word| lower.contains(word))
}
fn is_todo(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "待办",
        "下一步",
        "未完成",
        "todo",
        "next action",
        "remaining",
    ]
    .iter()
    .any(|word| lower.contains(word))
}
fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.into();
    }
    format!("{}…", value.chars().take(max).collect::<String>())
}

pub fn calculate_health(
    db_path: &Path,
    project_id: &str,
    nodes: &[ConversationNode],
    context_budget: usize,
) -> AppResult<ContextHealth> {
    let message_count = nodes
        .iter()
        .filter(|node| node.node_type == "message")
        .count();
    let estimated_tokens = nodes
        .iter()
        .map(|node| estimate(&node.content))
        .sum::<usize>();
    let mut counts = HashMap::<String, usize>::new();
    for node in nodes {
        *counts.entry(normalized(&node.content)).or_default() += 1;
    }
    let duplicates = counts
        .values()
        .map(|count| count.saturating_sub(1))
        .sum::<usize>();
    let duplicate_ratio = if nodes.is_empty() {
        0.0
    } else {
        duplicates as f64 / nodes.len() as f64
    };
    let total_chars = nodes
        .iter()
        .map(|node| node.content.len())
        .sum::<usize>()
        .max(1);
    let tool_chars = nodes
        .iter()
        .filter(|node| node.node_type == "tool_call")
        .map(|node| node.content.len())
        .sum::<usize>();
    let tool_log_ratio = tool_chars as f64 / total_chars as f64;
    let stale = nodes
        .iter()
        .filter(|node| matches!(node.status.as_str(), "stale" | "excluded"))
        .count();
    let stale_ratio = if nodes.is_empty() {
        0.0
    } else {
        stale as f64 / nodes.len() as f64
    };
    let incorrect = nodes
        .iter()
        .filter(|node| node.status == "incorrect")
        .count();
    let incorrect_ratio = if nodes.is_empty() {
        0.0
    } else {
        incorrect as f64 / nodes.len() as f64
    };
    let conflict_count = nodes
        .iter()
        .filter(|node| node.status == "incorrect" || node.node_type == "conflict")
        .count();
    let uncompressed_log_count = nodes
        .iter()
        .filter(|node| node.node_type == "tool_call" && node.content.chars().count() > 520)
        .count();
    let threshold_ratio = estimated_tokens as f64 / context_budget.max(1) as f64;
    let risk = threshold_ratio
        + duplicate_ratio * 0.35
        + tool_log_ratio * 0.2
        + stale_ratio * 0.2
        + incorrect_ratio * 0.3
        + (conflict_count.min(5) as f64 * 0.03);
    let level = if risk < 0.45 {
        ContextHealthLevel::Healthy
    } else if risk < 0.65 {
        ContextHealthLevel::Growing
    } else if risk < 0.82 {
        ContextHealthLevel::CompressionRecommended
    } else if risk < 1.05 {
        ContextHealthLevel::FreshContinuationRecommended
    } else {
        ContextHealthLevel::Critical
    };
    let conn = database::connect(db_path)?;
    let last_snapshot_at=conn.query_row("SELECT created_at FROM context_snapshots WHERE project_id=?1 ORDER BY created_at DESC LIMIT 1",params![project_id],|row|row.get(0)).optional()?;
    let last_fresh_continuation_at=conn.query_row("SELECT updated_at FROM continuations WHERE project_id=?1 AND mode='context' AND status IN ('listening','completed') ORDER BY updated_at DESC LIMIT 1",params![project_id],|row|row.get(0)).optional()?;
    let session_started:Option<String>=conn.query_row("SELECT ss.created_at FROM source_sessions ss JOIN project_bindings pb ON pb.binding_id=ss.id AND pb.binding_type='source_session' WHERE pb.project_id=?1 ORDER BY ss.created_at DESC LIMIT 1",params![project_id],|row|row.get(0)).optional()?;
    let current_session_duration_seconds = session_started
        .as_deref()
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
        .map(|value| (Utc::now().timestamp() - value.timestamp()).max(0));
    let mut reasons = Vec::new();
    if threshold_ratio >= 0.72 {
        reasons.push(format!(
            "估算上下文已达到预算的 {:.0}%",
            threshold_ratio * 100.0
        ));
    }
    if duplicate_ratio >= 0.12 {
        reasons.push(format!("重复内容比例约 {:.0}%", duplicate_ratio * 100.0));
    }
    if tool_log_ratio >= 0.25 {
        reasons.push(format!("工具输出占比约 {:.0}%", tool_log_ratio * 100.0));
    }
    if stale_ratio > 0.0 {
        reasons.push(format!("存在 {stale} 条过期或排除内容"));
    }
    if incorrect > 0 {
        reasons.push(format!("存在 {incorrect} 条已标记错误内容"));
    }
    if conflict_count > 0 {
        reasons.push(format!("存在 {conflict_count} 个显式冲突"));
    }
    if reasons.is_empty() {
        reasons.push("当前上下文规模和重复比例处于可控范围".into());
    }
    Ok(ContextHealth {
        level,
        message_count,
        estimated_tokens,
        duplicate_ratio,
        tool_log_ratio,
        stale_ratio,
        incorrect_ratio,
        conflict_count,
        uncompressed_log_count,
        context_budget,
        threshold_ratio,
        last_snapshot_at,
        last_fresh_continuation_at,
        current_session_duration_seconds,
        reasons,
    })
}

impl ContextCompressionProvider for RuleBasedProvider {
    fn compile(
        &self,
        db_path: &Path,
        options: &ContextCompileOptions,
    ) -> AppResult<CompiledContext> {
        if options.token_budget < 1000 {
            return Err(AppError::Message("上下文预算至少为 1000 Token".into()));
        }
        let project = unified_project::get(db_path, &options.project_id, options.token_budget)?;
        let mut nodes =
            unified_project::timeline(db_path, &options.project_id, &options.branch_id)?;
        if let Some(source) = options.source_node_id.as_deref() {
            let position = nodes
                .iter()
                .position(|node| node.id == source)
                .ok_or_else(|| AppError::Message("续接起点不在当前分支".into()))?;
            nodes.truncate(position + 1);
        }
        let health = calculate_health(db_path, &options.project_id, &nodes, options.token_budget)?;
        let recent_message_ids = nodes
            .iter()
            .rev()
            .filter(|node| node.node_type == "message")
            .take(options.recent_rounds.saturating_mul(2))
            .map(|node| node.id.clone())
            .collect::<HashSet<_>>();
        let mut seen = HashSet::new();
        let mut items = Vec::new();
        items.push(item(
            None,
            "project_goal",
            "keep",
            "项目总体目标始终保留",
            &format!("项目目标：{}", project.summary.goal),
            true,
        ));
        for constraint in &project.constraints {
            items.push(item(
                None,
                "user_constraint",
                "keep",
                "用户在统一项目中明确固定的长期约束",
                &format!("约束：{constraint}"),
                true,
            ));
        }
        let conn = database::connect(db_path)?;
        if options.include_skills {
            let mut stmt=conn.prepare("SELECT s.name,s.description,s.source_platform FROM project_bindings pb JOIN skills s ON s.id=pb.binding_id WHERE pb.project_id=?1 AND pb.binding_type='skill' ORDER BY s.name")?;
            let bound = stmt
                .query_map(params![options.project_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            for (name, description, platform) in bound {
                items.push(item(
                    None,
                    "skill",
                    "keep",
                    "用户已将 Skill 绑定到当前统一项目",
                    &format!(
                        "可用 Skill：{name}（{platform}）— {}",
                        truncate(&description, 320)
                    ),
                    false,
                ));
            }
        }
        if options.include_mcp {
            let mut stmt=conn.prepare("SELECT m.name,m.transport,m.command,m.source_agent FROM project_bindings pb JOIN mcp_servers m ON m.id=pb.binding_id WHERE pb.project_id=?1 AND pb.binding_type='mcp' ORDER BY m.name")?;
            let bound = stmt
                .query_map(params![options.project_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            for (name, transport, command, source) in bound {
                items.push(item(
                    None,
                    "mcp_server",
                    "keep",
                    "用户已将 MCP Server 绑定到当前统一项目",
                    &format!(
                        "可用 MCP：{name}；来源 {source}；传输 {transport}；命令 {}",
                        command.as_deref().unwrap_or("未公开")
                    ),
                    false,
                ));
            }
        }
        drop(conn);
        for node in &nodes {
            let key = normalized(&node.content);
            let duplicate = !key.is_empty() && !seen.insert(key);
            let mut category = "stale_information";
            let mut action = "retrieve_only";
            let mut reason = "较旧内容保留在可检索历史，不直接注入".to_owned();
            let mut content = node.content.clone();
            let pinned = node.importance >= 90;
            if duplicate {
                action = "exclude";
                reason = "与更早保留内容重复".into();
            } else if matches!(node.status.as_str(), "stale" | "incorrect" | "excluded") {
                action = "exclude";
                reason = format!("节点已标记为 {}", node.status);
            } else if node.node_type == "constraint" || is_constraint(&node.content) {
                category = "user_constraint";
                action = "keep";
                reason = "用户明确约束".into();
            } else if node.node_type == "decision" || pinned {
                category = if pinned {
                    "user_pinned_item"
                } else {
                    "architecture_decision"
                };
                action = "keep";
                reason = if pinned {
                    "用户固定或高重要度信息".into()
                } else {
                    "重要决策".into()
                };
            } else if node.node_type == "error" {
                category = "failed_attempt";
                if options.include_failed_attempts {
                    action = "keep";
                    reason = "最近失败与错误需要避免重复".into();
                } else {
                    action = "retrieve_only";
                    reason = "用户选择不注入失败尝试".into();
                }
            } else if node.node_type == "file_change"
                || node.node_type == "todo"
                || is_todo(&node.content)
            {
                category = if node.node_type == "file_change" {
                    "active_file"
                } else {
                    "remaining_task"
                };
                action = "keep";
                reason = "当前阶段的活跃文件或未完成任务".into();
            } else if node.node_type == "tool_call" {
                category = if node.content.to_ascii_lowercase().contains("test") {
                    "test_result"
                } else {
                    "tool_result"
                };
                if !options.include_tool_logs {
                    action = "exclude";
                    reason = "用户选择不包含工具日志".into();
                } else if node.content.chars().count() > 520 {
                    action = "compress";
                    reason = "工具输出超过 520 字符限制".into();
                    content = format!(
                        "[工具输出已压缩] 来源节点 {}；原始字符 {}；工具摘要：{}",
                        node.id,
                        node.content.chars().count(),
                        truncate(node.content.lines().next().unwrap_or("工具调用"), 180)
                    );
                } else {
                    action = "keep";
                    reason = "近期且长度可控的工具记录".into();
                }
            } else if recent_message_ids.contains(&node.id) {
                category = "recent_message";
                action = "keep";
                reason = "最近 N 轮完整消息".into();
            } else if node.node_type == "session_switch"
                || node.node_type == "summary"
                || node.node_type == "branch_point"
            {
                category = "current_state";
                action = "compress";
                reason = "保留会话或分支边界，但压缩为结构标记".into();
                content = truncate(&node.content, 240);
            } else if node.status == "completed" {
                action = "exclude";
                reason = "所属过程已经完成且不再直接相关".into();
            }
            items.push(item(
                Some(node.id.clone()),
                category,
                action,
                &reason,
                &content,
                pinned,
            ));
        }
        let git = git_inspector::inspect(Path::new(&project.summary.project_path));
        let mut git_text = format!(
            "Git 分支：{}\nHEAD：{}\n已修改：{}\n已暂存：{}\n未跟踪：{}",
            git.branch.as_deref().unwrap_or("未知"),
            git.head.as_deref().unwrap_or("未知"),
            git.modified.join(", "),
            git.staged.join(", "),
            git.untracked.join(", ")
        );
        if options.include_git_diff && !git.working_tree_diff.is_empty() {
            git_text.push_str("\nDiff 摘要：\n");
            git_text.push_str(&truncate(&git.working_tree_diff, 1600));
        }
        items.push(item(
            None,
            "git_state",
            "keep",
            "续接前读取的当前工作区真实 Git 状态",
            &git_text,
            false,
        ));
        apply_overrides(db_path, options, &mut items)?;
        let mut current = items
            .iter()
            .filter(|item| matches!(item.action.as_str(), "keep" | "compress"))
            .map(|item| item.estimated_tokens)
            .sum::<usize>();
        if current > options.token_budget {
            for item in items.iter_mut().rev() {
                if current <= options.token_budget {
                    break;
                }
                if matches!(item.category.as_str(), "recent_message" | "tool_result")
                    && item.action == "keep"
                    && !item.pinned
                {
                    current = current.saturating_sub(item.estimated_tokens);
                    item.action = "retrieve_only".into();
                    item.reason = "上下文预算不足，降级为可检索历史".into();
                }
            }
        }
        let system_context=format!("你正在通过 Continuum 续接一个跨 Agent 的统一项目。目标 Agent：{:?}，模型：{}。先核对磁盘和 Git 状态；来源会话记录可能已过期，不要自动执行历史命令。",options.target_agent,options.target_model);
        let compiled_text = render(&system_context, &items);
        let estimated_tokens = estimate(&compiled_text);
        let content_hash = hash_text(&compiled_text);
        let conflicts = nodes
            .iter()
            .filter(|node| node.status == "incorrect")
            .map(|node| {
                format!(
                    "节点 {} 已标记为错误：{}",
                    node.id,
                    truncate(&node.content, 100)
                )
            })
            .collect();
        Ok(CompiledContext {
            project_id: options.project_id.clone(),
            branch_id: options.branch_id.clone(),
            target_agent: options.target_agent.clone(),
            target_model: options.target_model.clone(),
            token_budget: options.token_budget,
            estimated_tokens,
            original_estimated_tokens: health.estimated_tokens,
            content_hash,
            generated_at: Utc::now().to_rfc3339(),
            system_context,
            compiled_text,
            items,
            conflicts,
            health,
        })
    }
}

fn apply_overrides(
    db_path: &Path,
    options: &ContextCompileOptions,
    items: &mut [ContextItem],
) -> AppResult<()> {
    let conn = database::connect(db_path)?;
    let mut stmt = conn.prepare(
        "SELECT source_node_id,content_hash,action,priority,pinned,stale,incorrect,permanent \
         FROM context_item_overrides \
         WHERE project_id=?1 AND (branch_id IS NULL OR branch_id=?2) \
         ORDER BY CASE WHEN branch_id IS NULL THEN 0 ELSE 1 END, updated_at",
    )?;
    let overrides = stmt
        .query_map(params![options.project_id, options.branch_id], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<i32>>(3)?,
                row.get::<_, Option<bool>>(4)?,
                row.get::<_, Option<bool>>(5)?,
                row.get::<_, Option<bool>>(6)?,
                row.get::<_, bool>(7)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for (source_node_id, content_hash, action, priority, pinned, stale, incorrect, permanent) in
        overrides
    {
        for item in items.iter_mut().filter(|item| {
            source_node_id
                .as_deref()
                .zip(item.source_node_id.as_deref())
                .is_some_and(|(left, right)| left == right)
                || (!content_hash.is_empty() && content_hash == item.content_hash)
        }) {
            if let Some(action) = action.as_deref() {
                item.action = action.into();
                item.reason = "由 Context Inspector 覆盖".into();
            }
            if let Some(priority) = priority {
                item.priority = priority;
            }
            if let Some(pinned) = pinned {
                item.pinned = pinned;
            }
            if let Some(stale) = stale {
                item.stale = stale;
            }
            if let Some(incorrect) = incorrect {
                item.incorrect = incorrect;
            }
            item.permanent = permanent;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn set_item_override(
    db_path: &Path,
    project_id: &str,
    branch_id: Option<&str>,
    source_node_id: Option<&str>,
    content_hash: &str,
    action: Option<&str>,
    priority: Option<i32>,
    pinned: Option<bool>,
    stale: Option<bool>,
    incorrect: Option<bool>,
    permanent: bool,
) -> AppResult<()> {
    if source_node_id.is_none() && content_hash.is_empty() {
        return Err(AppError::Message("Context 覆盖缺少稳定标识".into()));
    }
    if let Some(value) = action {
        if !matches!(value, "keep" | "compress" | "retrieve_only" | "exclude") {
            return Err(AppError::Message(format!("不支持的 Context 操作：{value}")));
        }
    }
    if priority.is_some_and(|value| !(0..=100).contains(&value)) {
        return Err(AppError::Message(
            "Context 优先级必须在 0 到 100 之间".into(),
        ));
    }
    let mut conn = database::connect(db_path)?;
    let tx = conn.transaction()?;
    tx.execute(
        "DELETE FROM context_item_overrides WHERE project_id=?1 AND branch_id IS ?2 AND source_node_id IS ?3 AND content_hash=?4",
        params![project_id, branch_id, source_node_id, content_hash],
    )?;
    let now = Utc::now().to_rfc3339();
    tx.execute(
        "INSERT INTO context_item_overrides(id,project_id,branch_id,source_node_id,content_hash,action,priority,pinned,stale,incorrect,permanent,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?12)",
        params![uuid::Uuid::new_v4().to_string(), project_id, branch_id, source_node_id, content_hash, action, priority, pinned, stale, incorrect, permanent, now],
    )?;
    tx.commit()?;
    Ok(())
}

fn item(
    source_node_id: Option<String>,
    category: &str,
    action: &str,
    reason: &str,
    content: &str,
    pinned: bool,
) -> ContextItem {
    let content_hash = hash_text(content);
    let id = hash_text(&format!(
        "{}|{category}|{action}|{content_hash}",
        source_node_id.as_deref().unwrap_or("generated")
    ));
    let priority = if pinned || matches!(category, "project_goal" | "user_constraint") {
        100
    } else if matches!(
        category,
        "remaining_task" | "known_issue" | "failed_attempt" | "git_state"
    ) {
        80
    } else {
        50
    };
    ContextItem {
        id,
        source_node_id,
        category: category.into(),
        action: action.into(),
        reason: reason.into(),
        estimated_tokens: estimate(content),
        content: content.into(),
        pinned,
        priority,
        stale: category == "stale_information",
        incorrect: category == "conflict",
        permanent: pinned || matches!(category, "project_goal" | "user_constraint"),
        content_hash,
    }
}

fn hash_text(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    hex::encode(hasher.finalize())
}
fn render(system: &str, items: &[ContextItem]) -> String {
    let mut output = format!("# System Context\n{system}\n");
    for (categories, title) in [
        (
            &[
                "project_goal",
                "user_constraint",
                "architecture_decision",
                "user_pinned_item",
            ][..],
            "Project Goal & Permanent Context",
        ),
        (
            &[
                "current_state",
                "remaining_task",
                "failed_attempt",
                "known_issue",
                "active_file",
                "git_state",
                "test_result",
                "skill",
                "mcp_server",
                "custom_instruction",
                "conflict",
            ][..],
            "Current Phase",
        ),
        (
            &["recent_message", "tool_result"][..],
            "Recent Conversation",
        ),
    ] {
        let selected = items
            .iter()
            .filter(|item| {
                categories.contains(&item.category.as_str())
                    && matches!(item.action.as_str(), "keep" | "compress")
            })
            .collect::<Vec<_>>();
        if selected.is_empty() {
            continue;
        }
        output.push_str(&format!("\n# {title}\n"));
        for item in selected {
            output.push_str("- ");
            output.push_str(&item.content);
            output.push('\n');
        }
    }
    let refs = items
        .iter()
        .filter(|item| matches!(item.action.as_str(), "retrieve_only" | "exclude"))
        .filter_map(|item| item.source_node_id.as_deref())
        .collect::<Vec<_>>();
    if !refs.is_empty() {
        output.push_str("\n# Omitted History References\n");
        output.push_str(&refs.join(", "));
        output.push('\n');
    }
    output
}

pub fn compile(db_path: &Path, options: &ContextCompileOptions) -> AppResult<CompiledContext> {
    RuleBasedProvider.compile(db_path, options)
}
pub fn save_snapshot(
    db_path: &Path,
    options: &ContextCompileOptions,
) -> AppResult<ContextSnapshot> {
    let compiled = compile(db_path, options)?;
    let id = uuid::Uuid::new_v4().to_string();
    let conn = database::connect(db_path)?;
    let json = serde_json::to_string(&compiled)?;
    conn.execute("INSERT INTO context_snapshots(id,project_id,branch_id,source_node_id,target_agent,target_model,token_budget,estimated_tokens,compiled_context,compiled_json,created_at,estimated_original_tokens,estimated_compiled_tokens,compiler_version,content_hash) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,'rule-v2',?14)",params![id,options.project_id,options.branch_id,options.source_node_id,format!("{:?}",options.target_agent).to_lowercase(),options.target_model,options.token_budget as i64,compiled.estimated_tokens as i64,compiled.compiled_text,json,compiled.generated_at,compiled.original_estimated_tokens as i64,compiled.estimated_tokens as i64,compiled.content_hash])?;
    for item in &compiled.items {
        let row_id = format!("{id}:{}", item.id);
        conn.execute("INSERT INTO context_items(id,snapshot_id,source_node_id,category,action,reason,estimated_tokens,content,pinned,action_reason,priority,stale,incorrect,permanent,content_hash) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",params![row_id,id,item.source_node_id,item.category,item.action,item.reason,item.estimated_tokens as i64,item.content,item.pinned,item.reason,item.priority,item.stale,item.incorrect,item.permanent,item.content_hash])?;
    }
    Ok(ContextSnapshot {
        id,
        source_node_id: options.source_node_id.clone(),
        compiled,
    })
}

pub fn diff_snapshots(
    db_path: &Path,
    from_snapshot_id: &str,
    to_snapshot_id: &str,
) -> AppResult<ContextSnapshotDiff> {
    let conn = database::connect(db_path)?;
    let load = |snapshot_id: &str| -> AppResult<CompiledContext> {
        let json: String = conn
            .query_row(
                "SELECT compiled_json FROM context_snapshots WHERE id=?1",
                params![snapshot_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| AppError::Message(format!("Context Snapshot 不存在：{snapshot_id}")))?;
        Ok(serde_json::from_str(&json)?)
    };
    let from = load(from_snapshot_id)?;
    let to = load(to_snapshot_id)?;
    if from.project_id != to.project_id {
        return Err(AppError::Message(
            "只能比较同一统一项目的 Context Snapshot".into(),
        ));
    }
    let from_items = from
        .items
        .iter()
        .map(|item| (item.id.as_str(), item))
        .collect::<HashMap<_, _>>();
    let to_items = to
        .items
        .iter()
        .map(|item| (item.id.as_str(), item))
        .collect::<HashMap<_, _>>();
    let added = to
        .items
        .iter()
        .filter(|item| !from_items.contains_key(item.id.as_str()))
        .cloned()
        .collect();
    let removed = from
        .items
        .iter()
        .filter(|item| !to_items.contains_key(item.id.as_str()))
        .cloned()
        .collect();
    let changed = to
        .items
        .iter()
        .filter(|item| {
            from_items.get(item.id.as_str()).is_some_and(|previous| {
                previous.content_hash != item.content_hash
                    || previous.action != item.action
                    || previous.priority != item.priority
                    || previous.pinned != item.pinned
            })
        })
        .cloned()
        .collect();
    Ok(ContextSnapshotDiff {
        from_snapshot_id: from_snapshot_id.into(),
        to_snapshot_id: to_snapshot_id.into(),
        added,
        removed,
        changed,
        token_delta: to.estimated_tokens as i64 - from.estimated_tokens as i64,
    })
}
pub fn list_snapshots(db_path: &Path, project_id: &str) -> AppResult<Vec<ContextSnapshot>> {
    let conn = database::connect(db_path)?;
    let mut stmt=conn.prepare("SELECT id,source_node_id,compiled_json FROM context_snapshots WHERE project_id=?1 ORDER BY created_at DESC")?;
    let rows = stmt
        .query_map(params![project_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter()
        .map(|(id, source, json)| {
            Ok(ContextSnapshot {
                id,
                source_node_id: source,
                compiled: serde_json::from_str(&json)?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    #[test]
    fn deterministic_rules_deduplicate_and_compress_tools() {
        let nodes = [
            ConversationNode {
                id: "a".into(),
                project_id: "p".into(),
                parent_node_id: None,
                branch_id: "b".into(),
                source_agent: Some(AgentKind::Codex),
                source_session_id: Some("s".into()),
                node_type: "message".into(),
                content: "Do not delete files".into(),
                created_at: "1".into(),
                importance: 50,
                status: "active".into(),
                metadata: BTreeMap::new(),
            },
            ConversationNode {
                id: "b".into(),
                project_id: "p".into(),
                parent_node_id: Some("a".into()),
                branch_id: "b".into(),
                source_agent: Some(AgentKind::Codex),
                source_session_id: Some("s".into()),
                node_type: "message".into(),
                content: "Do not delete files".into(),
                created_at: "2".into(),
                importance: 50,
                status: "active".into(),
                metadata: BTreeMap::new(),
            },
        ];
        assert!(is_constraint(&nodes[0].content));
        let mut seen = HashSet::new();
        assert!(seen.insert(normalized(&nodes[0].content)));
        assert!(!seen.insert(normalized(&nodes[1].content)));
        assert!(estimate(&"x".repeat(400)) >= 100);
    }
}
