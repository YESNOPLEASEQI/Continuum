use crate::{
    database,
    error::{AppError, AppResult},
    models::*,
};
use regex::Regex;
use rusqlite::params;
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

fn id_for(prefix: &str, value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("{prefix}_{}", &hex::encode(hasher.finalize())[..16])
}
fn bound(db_path: &Path, project_id: Option<&str>, kind: &str, id: &str) -> AppResult<bool> {
    let Some(project_id) = project_id else {
        return Ok(false);
    };
    let count:i64=database::connect(db_path)?.query_row("SELECT COUNT(*) FROM project_bindings WHERE project_id=?1 AND binding_type=?2 AND binding_id=?3",params![project_id,kind,id],|row|row.get(0))?;
    Ok(count > 0)
}
fn frontmatter(text: &str, key: &str) -> Option<String> {
    let mut lines = text.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }
    for line in lines {
        if line.trim() == "---" {
            break;
        }
        if let Some(value) = line.strip_prefix(&format!("{key}:")) {
            return Some(value.trim().trim_matches('"').to_owned());
        }
    }
    None
}

pub fn scan(db_path: &Path, project_id: Option<&str>) -> AppResult<ConfigurationInventory> {
    let home = dirs::home_dir().ok_or_else(|| AppError::Message("无法确定用户主目录".into()))?;
    let mut skills = Vec::new();
    for root in [
        home.join(".codex").join("skills"),
        home.join(".agents").join("skills"),
    ] {
        if !root.is_dir() {
            continue;
        }
        for entry in walkdir::WalkDir::new(&root)
            .max_depth(3)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
        {
            if !entry.file_type().is_file()
                || entry.file_name().to_string_lossy().to_ascii_lowercase() != "skill.md"
            {
                continue;
            }
            let text = fs::read_to_string(entry.path()).unwrap_or_default();
            let path = entry.path().to_string_lossy().into_owned();
            let id = id_for("skill", &path);
            let name = frontmatter(&text, "name")
                .or_else(|| {
                    entry
                        .path()
                        .parent()
                        .and_then(|path| path.file_name())
                        .map(|value| value.to_string_lossy().into_owned())
                })
                .unwrap_or_else(|| "Unnamed skill".into());
            let description = frontmatter(&text, "description").unwrap_or_else(|| {
                text.lines()
                    .find(|line| {
                        !line.trim().is_empty()
                            && !line.starts_with("---")
                            && !line.starts_with('#')
                    })
                    .unwrap_or("未提供描述")
                    .trim()
                    .into()
            });
            let skill = UnifiedSkill {
                id: id.clone(),
                name,
                description,
                source_platform: "codex".into(),
                source_path: path.clone(),
                compatible_agents: vec![AgentKind::Codex],
                required_tools: Vec::new(),
                instructions: text,
                installation_state: "available".into(),
                bound: bound(db_path, project_id, "skill", &id)?,
            };
            let conn = database::connect(db_path)?;
            conn.execute("INSERT INTO skills(id,name,description,source_platform,source_path,compatible_agents_json,required_tools_json,instructions,installation_state,discovered_at) VALUES(?1,?2,?3,?4,?5,?6,'[]',?7,?8,?9) ON CONFLICT(id) DO UPDATE SET name=excluded.name,description=excluded.description,instructions=excluded.instructions,discovered_at=excluded.discovered_at",params![skill.id,skill.name,skill.description,skill.source_platform,skill.source_path,serde_json::to_string(&skill.compatible_agents)?,skill.instructions,skill.installation_state,chrono::Utc::now().to_rfc3339()])?;
            skills.push(skill);
        }
    }
    let mut mcp_servers = Vec::new();
    let config = home.join(".codex").join("config.toml");
    if config.is_file() {
        let text = fs::read_to_string(&config)?;
        let section =
            Regex::new(r"(?m)^\[mcp_servers\.([^\]]+)\]\s*$").expect("valid MCP section regex");
        let command =
            Regex::new(r#"(?m)^command\s*=\s*["']([^"']+)["']"#).expect("valid MCP command regex");
        let captures = section.captures_iter(&text).collect::<Vec<_>>();
        for (index, capture) in captures.iter().enumerate() {
            let name = capture.get(1).unwrap().as_str().to_owned();
            let start = capture.get(0).unwrap().end();
            let end = captures
                .get(index + 1)
                .and_then(|next| next.get(0))
                .map(|value| value.start())
                .unwrap_or(text.len());
            let body = &text[start..end];
            let cmd = command
                .captures(body)
                .and_then(|capture| capture.get(1))
                .map(|value| value.as_str().to_owned());
            let id = id_for("mcp", &format!("codex:{name}"));
            let item = McpServerInfo {
                id: id.clone(),
                name,
                source_agent: AgentKind::Codex,
                command: cmd,
                transport: "stdio".into(),
                compatible_agents: vec![AgentKind::Codex],
                bound: bound(db_path, project_id, "mcp", &id)?,
            };
            database::connect(db_path)?.execute("INSERT INTO mcp_servers(id,name,source_agent,command,transport,compatible_agents_json,discovered_at) VALUES(?1,?2,'codex',?3,?4,?5,?6) ON CONFLICT(id) DO UPDATE SET command=excluded.command,discovered_at=excluded.discovered_at",params![item.id,item.name,item.command,item.transport,serde_json::to_string(&item.compatible_agents)?,chrono::Utc::now().to_rfc3339()])?;
            mcp_servers.push(item);
        }
    }
    let mut custom_instructions = Vec::new();
    for path in [
        home.join(".codex").join("AGENTS.md"),
        home.join("AGENTS.md"),
    ] {
        if path.is_file() {
            let value = path.to_string_lossy().into_owned();
            custom_instructions.push(CustomInstructionInfo {
                id: id_for("instructions", &value),
                name: path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                path: value,
                source_agent: AgentKind::Codex,
            });
        }
    }
    skills.sort_by(|a, b| a.name.cmp(&b.name));
    mcp_servers.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(ConfigurationInventory {
        skills,
        mcp_servers,
        custom_instructions,
    })
}

pub fn set_binding(
    db_path: &Path,
    project_id: &str,
    kind: &str,
    item_id: &str,
    is_bound: bool,
) -> AppResult<()> {
    if !matches!(kind, "skill" | "mcp") {
        return Err(AppError::Message("配置绑定类型无效".into()));
    }
    let conn = database::connect(db_path)?;
    if is_bound {
        let exists: i64 = match kind {
            "skill" => conn.query_row(
                "SELECT COUNT(*) FROM skills WHERE id=?1",
                params![item_id],
                |row| row.get(0),
            )?,
            _ => conn.query_row(
                "SELECT COUNT(*) FROM mcp_servers WHERE id=?1",
                params![item_id],
                |row| row.get(0),
            )?,
        };
        if exists == 0 {
            return Err(AppError::Message("找不到要绑定的配置".into()));
        }
        conn.execute("INSERT OR IGNORE INTO project_bindings(project_id,binding_type,binding_id,created_at,metadata_json) VALUES(?1,?2,?3,?4,'{}')",params![project_id,kind,item_id,chrono::Utc::now().to_rfc3339()])?;
    } else {
        conn.execute("DELETE FROM project_bindings WHERE project_id=?1 AND binding_type=?2 AND binding_id=?3",params![project_id,kind,item_id])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_frontmatter_fields() {
        let text = "---\nname: useful-skill\ndescription: Does work\n---\n# Body";
        assert_eq!(frontmatter(text, "name").as_deref(), Some("useful-skill"));
        assert_eq!(
            frontmatter(text, "description").as_deref(),
            Some("Does work")
        );
    }
}
