use crate::models::SecurityFinding;
use regex::Regex;
use serde_json::Value;

struct SecretPattern {
    name: &'static str,
    regex: Regex,
    severity: &'static str,
}

fn patterns() -> Vec<SecretPattern> {
    [
        ("OpenAI API Key", r"sk-[A-Za-z0-9_-]{20,}", "high"),
        ("Anthropic API Key", r"sk-ant-[A-Za-z0-9_-]{20,}", "high"),
        ("GitHub Token", r"gh[opsu]_[A-Za-z0-9]{20,}", "high"),
        ("AWS Access Key", r"AKIA[0-9A-Z]{16}", "high"),
        (
            "Bearer Token",
            r"(?i)Bearer\s+[A-Za-z0-9._~+/-]{12,}=*",
            "high",
        ),
        (
            "Private Key",
            r"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----",
            "high",
        ),
        (
            "Environment File",
            r"(?i)(?:^|[/\\])\.env(?:\.[A-Za-z0-9_-]+)?(?:$|\s)",
            "medium",
        ),
        (
            "Authorization Header",
            r"(?i)Authorization\s*:\s*[^\s,;]{8,}",
            "high",
        ),
        (
            "Cookie",
            r"(?i)(?:Cookie|Set-Cookie)\s*:\s*[^\r\n]{8,}",
            "medium",
        ),
        (
            "Password Field",
            r#"(?i)(password|passwd|pwd)\s*[=:]\s*["']?[^\s,"']{6,}"#,
            "high",
        ),
    ]
    .into_iter()
    .map(|(name, pattern, severity)| SecretPattern {
        name,
        regex: Regex::new(pattern).expect("valid secret regex"),
        severity,
    })
    .collect()
}

pub fn redact_text(
    text: &str,
    source_file: &str,
    field_path: &str,
) -> (String, Vec<SecurityFinding>) {
    let mut output = text.to_owned();
    let mut findings = Vec::new();
    for pattern in patterns() {
        if pattern.regex.is_match(&output) {
            findings.push(SecurityFinding {
                finding_type: pattern.name.into(),
                source_file: source_file.into(),
                field_path: field_path.into(),
                severity: pattern.severity.into(),
            });
            output = pattern
                .regex
                .replace_all(&output, "[REDACTED]")
                .into_owned();
        }
    }
    (output, findings)
}

pub fn redact_value(value: &mut Value, source_file: &str) -> Vec<SecurityFinding> {
    let mut findings = Vec::new();
    redact_node(value, source_file, "$", &mut findings);
    findings
}

pub fn redact_diagnostics_value(value: &mut Value) -> Vec<SecurityFinding> {
    let mut findings = redact_value(value, "diagnostics");
    let home = dirs::home_dir().map(|path| path.to_string_lossy().into_owned());
    let username = std::env::var("USERNAME")
        .ok()
        .filter(|value| !value.is_empty());
    fn scrub(value: &mut Value, home: Option<&str>, username: Option<&str>) {
        match value {
            Value::String(text) => {
                if let Some(home) = home {
                    *text = text.replace(home, "[HOME]");
                }
                if let Some(username) = username {
                    *text = text.replace(username, "[USER]");
                }
            }
            Value::Array(values) => values
                .iter_mut()
                .for_each(|value| scrub(value, home, username)),
            Value::Object(values) => values
                .values_mut()
                .for_each(|value| scrub(value, home, username)),
            _ => {}
        }
    }
    scrub(value, home.as_deref(), username.as_deref());
    findings.shrink_to_fit();
    findings
}
fn redact_node(
    value: &mut Value,
    source_file: &str,
    path: &str,
    findings: &mut Vec<SecurityFinding>,
) {
    match value {
        Value::String(text) => {
            let (redacted, mut found) = redact_text(text, source_file, path);
            *text = redacted;
            findings.append(&mut found);
        }
        Value::Array(items) => {
            for (index, item) in items.iter_mut().enumerate() {
                redact_node(item, source_file, &format!("{path}[{index}]"), findings);
            }
        }
        Value::Object(map) => {
            for (key, item) in map.iter_mut() {
                let field = format!("{path}.{key}");
                if matches!(
                    key.to_ascii_lowercase().as_str(),
                    "password"
                        | "passwd"
                        | "pwd"
                        | "token"
                        | "api_key"
                        | "apikey"
                        | "authorization"
                        | "cookie"
                ) {
                    if !item.is_null() {
                        findings.push(SecurityFinding {
                            finding_type: "Sensitive Field".into(),
                            source_file: source_file.into(),
                            field_path: field.clone(),
                            severity: "high".into(),
                        });
                        *item = Value::String("[REDACTED]".into());
                    }
                } else {
                    redact_node(item, source_file, &field, findings);
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn redacts_keys_without_leaking() {
        let input = "Authorization: Bearer abcdefghijklmnopqrstuvwxyz";
        let (output, findings) = redact_text(input, "session.jsonl", "$.message");
        assert!(!output.contains("abcdefghijklmnopqrstuvwxyz"));
        assert!(output.contains("[REDACTED]"));
        assert!(!findings.is_empty());
    }
    #[test]
    fn redacts_password_fields() {
        let mut value = serde_json::json!({"password":"super-secret-value","safe":"ok"});
        let findings = redact_value(&mut value, "goal.json");
        assert_eq!(value["password"], "[REDACTED]");
        assert_eq!(findings.len(), 1);
    }
}
