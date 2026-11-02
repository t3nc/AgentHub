use crate::l10n::{Msg, t};
use std::path::Path;

use serde_json::{Map, Value, json};

use crate::error::{Error, Result};
use crate::model::mcp::{McpServer, Transport};
use crate::text;

/// 读取 JSON 配置里已有的 MCP 条目名（用于「已存在」判断）。
pub fn existing_names(text: &str, pointer: &[String]) -> Vec<String> {
    let doc: Value = serde_json::from_str(text).unwrap_or_else(|_| json!({}));
    let mut node = &doc;
    for key in pointer {
        match node.get(key) {
            Some(next) => node = next,
            None => return Vec::new(),
        }
    }
    node.as_object()
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default()
}

/// 键级 upsert：只动 pointer 指向的那一层，其余键顺序与内容保持不变。
pub fn upsert(text: Option<&str>, pointer: &[String], servers: &[McpServer]) -> Result<String> {
    let mut doc: Value = match text {
        Some(t) if !t.trim().is_empty() => serde_json::from_str(t)?,
        _ => json!({}),
    };
    if !doc.is_object() {
        return Err(Error::Other(t!(Msg::ErrMcpJsonTopNotObject)));
    }
    {
        let mut node = &mut doc;
        for key in pointer {
            if !node.is_object() {
                return Err(Error::Other(t!(Msg::ErrMcpKeyNotObject, key = key)));
            }
            node = node
                .as_object_mut()
                .unwrap()
                .entry(key.clone())
                .or_insert_with(|| Value::Object(Map::new()));
        }
        let map = node
            .as_object_mut()
            .ok_or_else(|| Error::Other(t!(Msg::ErrMcpNodeNotObject)))?;
        for server in servers {
            map.insert(server.name.clone(), render_server(server));
        }
    }
    let rendered = serde_json::to_string_pretty(&doc).map_err(Error::Json)?;
    Ok(restore_eol(text, rendered))
}

/// `serde_json` 只会输出 LF 且不带结尾换行：按原文件风格还原，避免整文件 diff。
fn restore_eol(original: Option<&str>, rendered: String) -> String {
    let eol = original.map(text::eol_of).unwrap_or("\n");
    let want_trailing = original.map(|t| t.ends_with('\n')).unwrap_or(true);
    let mut out = text::with_eol(&rendered, eol);
    if want_trailing && !out.ends_with(eol) {
        out.push_str(eol);
    }
    out
}

fn render_server(server: &McpServer) -> Value {
    match &server.transport {
        Transport::Stdio {
            command,
            args,
            env,
            cwd,
        } => {
            let mut obj = Map::new();
            obj.insert("command".into(), json!(command));
            if !args.is_empty() {
                obj.insert("args".into(), json!(args));
            }
            let mut env_obj = Map::new();
            for (k, v) in env {
                env_obj.insert(k.clone(), json!(v.render_value()));
            }
            if !env_obj.is_empty() {
                obj.insert("env".into(), Value::Object(env_obj));
            }
            if let Some(cwd) = cwd {
                obj.insert("cwd".into(), json!(cwd));
            }
            Value::Object(obj)
        }
        Transport::Http { url, headers } => {
            let mut obj = Map::new();
            obj.insert("type".into(), json!("http"));
            obj.insert("url".into(), json!(url));
            let mut h = Map::new();
            for (k, v) in headers {
                h.insert(k.clone(), json!(v.render_value()));
            }
            if !h.is_empty() {
                obj.insert("headers".into(), Value::Object(h));
            }
            Value::Object(obj)
        }
    }
}

/// 把文件读成文本；不存在返回 None。
pub fn read_optional(path: &Path) -> Result<Option<String>> {
    if !path.exists() {
        return Ok(None);
    }
    crate::paths::read_to_string_limited(path, 8 * 1024 * 1024).map(Some)
}
