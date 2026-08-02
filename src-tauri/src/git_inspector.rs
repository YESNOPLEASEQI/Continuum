use crate::models::GitState;
use std::{
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

fn run_git(directory: &Path, args: &[&str], timeout: Duration) -> Result<String, String> {
    let mut child = Command::new("git")
        .args(args)
        .current_dir(directory)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("无法启动 Git：{error}"))?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                let output = child
                    .wait_with_output()
                    .map_err(|error| error.to_string())?;
                if output.status.success() {
                    return Ok(String::from_utf8_lossy(&output.stdout)
                        .trim_end()
                        .to_owned());
                }
                return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
            }
            Ok(None) if started.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("Git 命令在 {} 秒后超时", timeout.as_secs()));
            }
            Ok(None) => thread::sleep(Duration::from_millis(20)),
            Err(error) => return Err(error.to_string()),
        }
    }
}

pub fn inspect(directory: &Path) -> GitState {
    let timeout = Duration::from_secs(5);
    let root = match run_git(directory, &["rev-parse", "--show-toplevel"], timeout) {
        Ok(value) => value,
        Err(error) => {
            return GitState {
                error: Some(error),
                ..Default::default()
            }
        }
    };
    let mut state = GitState {
        is_repository: true,
        repository_path: Some(root.clone()),
        ..Default::default()
    };
    state.branch = run_git(directory, &["branch", "--show-current"], timeout)
        .ok()
        .filter(|v| !v.is_empty());
    state.head = run_git(directory, &["rev-parse", "HEAD"], timeout).ok();
    let status =
        run_git(directory, &["status", "--porcelain=v1", "-z"], timeout).unwrap_or_default();
    parse_porcelain(&status, &mut state);
    state.working_tree_diff =
        run_git(directory, &["diff", "--no-ext-diff", "--binary"], timeout).unwrap_or_default();
    state.staged_diff = run_git(
        directory,
        &["diff", "--cached", "--no-ext-diff", "--binary"],
        timeout,
    )
    .unwrap_or_default();
    state
}

pub fn parse_porcelain(input: &str, state: &mut GitState) {
    for record in input.split('\0').filter(|v| !v.is_empty()) {
        if record.len() < 3 {
            continue;
        }
        let bytes = record.as_bytes();
        let x = bytes[0] as char;
        let y = bytes[1] as char;
        let path = record[3..].to_owned();
        if x == '?' && y == '?' {
            state.untracked.push(path);
            continue;
        }
        if x != ' ' && x != '?' {
            state.staged.push(path.clone());
        }
        if y != ' ' && y != '?' {
            state.modified.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_git_status() {
        let mut state = GitState::default();
        parse_porcelain(" M src/main.rs\0A  added.txt\0?? draft.txt\0", &mut state);
        assert_eq!(state.modified, vec!["src/main.rs"]);
        assert_eq!(state.staged, vec!["added.txt"]);
        assert_eq!(state.untracked, vec!["draft.txt"]);
    }
}
