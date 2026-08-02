use crate::{
    database,
    error::{AppError, AppResult},
    models::GlobalSearchResult,
};
use rusqlite::params;
use std::path::Path;

fn excerpt(value: &str, query: &str) -> String {
    let value = value.replace(['\r', '\n'], " ");
    let lower = value.to_lowercase();
    let position = lower.find(&query.to_lowercase()).unwrap_or(0);
    let start = position.saturating_sub(60);
    value.chars().skip(start).take(220).collect()
}

pub fn global(db_path: &Path, query: &str, limit: usize) -> AppResult<Vec<GlobalSearchResult>> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(vec![]);
    }
    if query.chars().count() > 500 {
        return Err(AppError::Message("搜索内容过长".into()));
    }
    let limit = limit.clamp(1, 200);
    let pattern = format!("%{}%", query.replace('%', "\\%").replace('_', "\\_"));
    let conn = database::connect(db_path)?;
    let mut results = Vec::new();

    {
        let mut statement = conn.prepare("SELECT id,name,project_path,goal,created_at FROM projects WHERE deleted_at IS NULL AND (name LIKE ?1 ESCAPE '\\' OR project_path LIKE ?1 ESCAPE '\\' OR goal LIKE ?1 ESCAPE '\\') ORDER BY updated_at DESC LIMIT ?2")?;
        for row in statement.query_map(params![pattern, limit as i64], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })? {
            let (id, name, path, goal, created_at) = row?;
            results.push(GlobalSearchResult {
                kind: "project".into(),
                id: id.clone(),
                title: name,
                excerpt: excerpt(&format!("{path} {goal}"), query),
                project_id: Some(id),
                branch_id: None,
                session_id: None,
                path: Some(path),
                created_at: Some(created_at),
            });
        }
    }
    {
        let mut statement = conn.prepare("SELECT id,project_id,name,created_at FROM conversation_branches WHERE name LIKE ?1 ESCAPE '\\' ORDER BY updated_at DESC LIMIT ?2")?;
        for row in statement.query_map(params![pattern, limit as i64], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })? {
            let (id, project_id, name, created_at) = row?;
            results.push(GlobalSearchResult {
                kind: "branch".into(),
                id: id.clone(),
                title: name.clone(),
                excerpt: name,
                project_id: Some(project_id),
                branch_id: Some(id),
                session_id: None,
                path: None,
                created_at: Some(created_at),
            });
        }
    }
    {
        let mut statement = conn.prepare("SELECT id,title,working_directory,source_path,created_at FROM source_sessions WHERE title LIKE ?1 ESCAPE '\\' OR id LIKE ?1 ESCAPE '\\' OR working_directory LIKE ?1 ESCAPE '\\' OR detail_json LIKE ?1 ESCAPE '\\' ORDER BY updated_at DESC LIMIT ?2")?;
        for row in statement.query_map(params![pattern, limit as i64], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })? {
            let (id, title, cwd, path, created_at) = row?;
            results.push(GlobalSearchResult {
                kind: "session".into(),
                id: id.clone(),
                title,
                excerpt: cwd.clone().unwrap_or_default(),
                project_id: None,
                branch_id: None,
                session_id: Some(id),
                path: Some(path),
                created_at: Some(created_at),
            });
        }
    }
    {
        let mut statement = conn.prepare("SELECT id,project_id,branch_id,source_session_id,node_type,content,created_at,metadata_json FROM conversation_nodes WHERE content LIKE ?1 ESCAPE '\\' OR metadata_json LIKE ?1 ESCAPE '\\' ORDER BY created_at DESC LIMIT ?2")?;
        for row in statement.query_map(params![pattern, limit as i64], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
            ))
        })? {
            let (id, project_id, branch_id, session_id, node_type, content, created_at) = row?;
            let kind = match node_type.as_str() {
                "file_change" => "file",
                "tool_call" if content.to_lowercase().contains("test") => "test",
                "tool_call" => "command",
                "error" => "error",
                "decision" => "decision",
                _ => "message",
            };
            results.push(GlobalSearchResult {
                kind: kind.into(),
                id,
                title: node_type,
                excerpt: excerpt(&content, query),
                project_id: Some(project_id),
                branch_id: Some(branch_id),
                session_id,
                path: None,
                created_at: Some(created_at),
            });
        }
    }
    {
        let mut statement = conn.prepare("SELECT id,name,description,source_path,modified_at FROM skills WHERE name LIKE ?1 ESCAPE '\\' OR description LIKE ?1 ESCAPE '\\' OR source_path LIKE ?1 ESCAPE '\\' ORDER BY modified_at DESC LIMIT ?2")?;
        for row in statement.query_map(params![pattern, limit as i64], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        })? {
            let (id, name, description, path, modified_at) = row?;
            results.push(GlobalSearchResult {
                kind: "skill".into(),
                id,
                title: name,
                excerpt: excerpt(&description, query),
                project_id: None,
                branch_id: None,
                session_id: None,
                path: Some(path),
                created_at: modified_at,
            });
        }
    }
    {
        let mut statement = conn.prepare("SELECT id,name,command,source_path,modified_at FROM mcp_servers WHERE name LIKE ?1 ESCAPE '\\' OR command LIKE ?1 ESCAPE '\\' OR source_path LIKE ?1 ESCAPE '\\' ORDER BY modified_at DESC LIMIT ?2")?;
        for row in statement.query_map(params![pattern, limit as i64], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        })? {
            let (id, name, command, path, modified_at) = row?;
            results.push(GlobalSearchResult {
                kind: "mcp".into(),
                id,
                title: name,
                excerpt: excerpt(command.as_deref().unwrap_or(""), query),
                project_id: None,
                branch_id: None,
                session_id: None,
                path: Some(path),
                created_at: modified_at,
            });
        }
    }
    results.sort_by(|left, right| {
        right
            .created_at
            .cmp(&left.created_at)
            .then_with(|| left.kind.cmp(&right.kind))
    });
    results.truncate(limit);
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excerpt_is_bounded_and_centers_the_match() {
        let value = format!("{}needle{}", "a".repeat(100), "b".repeat(300));
        let result = excerpt(&value, "needle");
        assert!(result.contains("needle"));
        assert!(result.chars().count() <= 220);
    }
}
