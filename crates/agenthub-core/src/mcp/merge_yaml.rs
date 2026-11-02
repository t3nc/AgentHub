use crate::l10n::{Msg, t};
use saphyr::LoadableYamlNode;

use crate::error::{Error, Result};
use crate::model::mcp::{McpServer, Transport};
use crate::text;

/// Hermes 的 `config.yaml` 注释密集且层级很深，整文件重写会丢注释。
/// 这里只定位顶层 `mcp_servers:` 到下一个顶层键之间的行区间，做块级替换。
pub fn existing_names(text: &str, top_key: &str) -> Vec<String> {
    let (start, end) = match block_range(text, top_key) {
        Some(r) => r,
        None => return Vec::new(),
    };
    let mut names = Vec::new();
    for line in text.lines().take(end).skip(start + 1) {
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("  ") {
            if rest.starts_with(' ') {
                continue;
            }
            if let Some((name, _)) = rest.split_once(':') {
                let name = name.trim().trim_matches('"').trim_matches('\'');
                if !name.is_empty() {
                    names.push(name.to_string());
                }
            }
        }
    }
    names
}

pub fn upsert(text: Option<&str>, top_key: &str, servers: &[McpServer]) -> Result<String> {
    let original = text.unwrap_or("");
    // 保留原文件的换行风格：CRLF 文件不能因为一次 upsert 被整体改成 LF
    let eol = text::eol_of(original);
    let source = text::normalize_lf(original);
    let block = render_block(top_key, servers);

    let out = match block_range(&source, top_key) {
        Some((start, end)) => {
            let mut lines: Vec<&str> = source.lines().collect();
            lines.splice(start..end, block.lines());
            let mut joined = lines.join("\n");
            joined.push('\n');
            joined
        }
        None => {
            let mut base = source;
            if !base.is_empty() && !base.ends_with('\n') {
                base.push('\n');
            }
            base.push_str(&block);
            base
        }
    };

    let out = text::with_eol(&out, eol);
    validate(&out)?;
    Ok(out)
}

/// 校验写入后的 YAML 仍然可解析（用 saphyr，版本无关的入口）。
fn validate(text: &str) -> Result<()> {
    saphyr::Yaml::load_from_str(text)
        .map(|_| ())
        .map_err(|e| Error::Yaml(t!(Msg::ErrYamlInvalidAfterWrite, error = e)))
}

/// 返回顶层键所在的行区间 [start, end)（含键行，end 不含）。
fn block_range(text: &str, top_key: &str) -> Option<(usize, usize)> {
    let lines: Vec<&str> = text.lines().collect();
    let mut start = None;
    for (idx, line) in lines.iter().enumerate() {
        if line.starts_with(top_key) && line[top_key.len()..].trim_start().starts_with(':') {
            start = Some(idx);
            break;
        }
    }
    let start = start?;
    let mut end = lines.len();
    for (idx, line) in lines.iter().enumerate().skip(start + 1) {
        if line.trim().is_empty() || line.starts_with(' ') || line.starts_with('#') {
            continue;
        }
        if is_top_level_key(line) {
            end = idx;
            break;
        }
    }
    Some((start, end))
}

fn is_top_level_key(line: &str) -> bool {
    if line.starts_with('-') || line.starts_with(' ') {
        return false;
    }
    match line.split_once(':') {
        Some((key, _)) => {
            !key.is_empty()
                && key
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
        }
        None => false,
    }
}

fn render_block(top_key: &str, servers: &[McpServer]) -> String {
    let mut out = String::new();
    if servers.is_empty() {
        out.push_str(&format!("{top_key}: {{}}\n"));
        return out;
    }
    out.push_str(&format!("{top_key}:\n"));
    for server in servers {
        out.push_str(&format!("  {}:\n", quote_if_needed(&server.name)));
        match &server.transport {
            Transport::Stdio {
                command,
                args,
                env,
                cwd,
            } => {
                out.push_str(&format!("    command: {}\n", quote_if_needed(command)));
                if !args.is_empty() {
                    out.push_str("    args:\n");
                    for arg in args {
                        out.push_str(&format!("      - {}\n", quote_if_needed(arg)));
                    }
                }
                if let Some(cwd) = cwd {
                    out.push_str(&format!("    cwd: {}\n", quote_if_needed(cwd)));
                }
                if !env.is_empty() {
                    out.push_str("    env:\n");
                    for (k, v) in env {
                        out.push_str(&format!(
                            "      {}: {}\n",
                            k,
                            quote_if_needed(&v.render_value())
                        ));
                    }
                }
            }
            Transport::Http { url, headers } => {
                out.push_str(&format!("    url: {}\n", quote_if_needed(url)));
                if !headers.is_empty() {
                    out.push_str("    headers:\n");
                    for (k, v) in headers {
                        out.push_str(&format!(
                            "      {}: {}\n",
                            k,
                            quote_if_needed(&v.render_value())
                        ));
                    }
                }
            }
        }
        out.push_str(&format!("    enabled: {}\n", server.enabled));
        if let Some(t) = server.timeout_secs {
            out.push_str(&format!("    timeout: {t}\n"));
        }
    }
    out
}

/// 只在必要时加引号，保持与原文件风格一致。
fn quote_if_needed(raw: &str) -> String {
    let needs = raw.is_empty()
        || raw.chars().any(|c| {
            matches!(
                c,
                ':' | '#'
                    | '{'
                    | '}'
                    | '['
                    | ']'
                    | ','
                    | '&'
                    | '*'
                    | '!'
                    | '|'
                    | '>'
                    | '@'
                    | '`'
                    | '\''
                    | '"'
            )
        })
        || raw.starts_with('-')
        || raw != raw.trim();
    if needs {
        format!("\"{}\"", raw.replace('"', "\\\""))
    } else {
        raw.to_string()
    }
}
