use crate::l10n::{Msg, t};
use std::path::Path;

use toml_edit::{Array, DocumentMut, Item, Table, value};

use crate::error::{Error, Result};
use crate::model::mcp::{McpServer, Transport};
use crate::text;

/// Codex 的 `config.toml` 里还有 model_providers、notify 等大量用户内容，
/// 因此只做 `[mcp_servers.*]` 表级替换，其余字节由 toml_edit 原样保留。
pub fn existing_names(text: &str, table: &str) -> Vec<String> {
    let doc: DocumentMut = match text.parse() {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    doc.get(table)
        .and_then(|item| item.as_table_like())
        .map(|t| t.iter().map(|(k, _)| k.to_string()).collect())
        .unwrap_or_default()
}

pub fn upsert(text: Option<&str>, table: &str, servers: &[McpServer]) -> Result<String> {
    let mut doc: DocumentMut = match text {
        Some(t) if !t.trim().is_empty() => t
            .parse()
            .map_err(|e| Error::Toml(t!(Msg::ErrTomlParseFailed, error = e)))?,
        _ => DocumentMut::new(),
    };

    let mut root = Table::new();
    root.set_implicit(false);
    for server in servers {
        root.insert(&server.name, Item::Table(render_server(server)));
    }
    doc[table] = Item::Table(root);

    // toml_edit 输出 LF；原文件是 CRLF 时按原风格还原，避免整文件 diff
    let eol = text.map(text::eol_of).unwrap_or("\n");
    let mut out = text::with_eol(&doc.to_string(), eol);
    if eol != "\n" && !out.ends_with(eol) {
        out.push_str(eol);
    }
    Ok(out)
}

fn render_server(server: &McpServer) -> Table {
    let mut table = Table::new();
    match &server.transport {
        Transport::Stdio {
            command,
            args,
            env,
            cwd,
        } => {
            table.insert("command", value(command.clone()));
            if !args.is_empty() {
                let mut arr = Array::new();
                for a in args {
                    arr.push(a.clone());
                }
                table.insert("args", value(arr));
            }
            if let Some(cwd) = cwd {
                table.insert("cwd", value(cwd.clone()));
            }
            if !env.is_empty() {
                let mut env_table = Table::new();
                for (k, v) in env {
                    env_table.insert(k, value(v.render_value()));
                }
                table.insert("env", Item::Table(env_table));
            }
        }
        Transport::Http { url, headers } => {
            table.insert("url", value(url.clone()));
            if !headers.is_empty() {
                let mut h = Table::new();
                for (k, v) in headers {
                    h.insert(k, value(v.render_value()));
                }
                table.insert("headers", Item::Table(h));
            }
        }
    }
    table.insert("enabled", value(server.enabled));
    if let Some(t) = server.timeout_secs {
        table.insert("startup_timeout_sec", value(t as i64));
    }
    table
}

pub fn read_optional(path: &Path) -> Result<Option<String>> {
    if !path.exists() {
        return Ok(None);
    }
    crate::paths::read_to_string_limited(path, 8 * 1024 * 1024).map(Some)
}
