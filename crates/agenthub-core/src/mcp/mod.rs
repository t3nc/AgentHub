pub mod merge_json;
pub mod merge_toml;
pub mod merge_yaml;
pub mod secrets;

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::Value;
use toml_edit::DocumentMut;

use crate::error::Result;
use crate::model::mcp::{McpFile, McpFormat, McpServer, SecretRef, Transport};
use crate::paths;

pub fn read_text(path: &Path) -> Result<Option<String>> {
    if !path.exists() {
        return Ok(None);
    }
    paths::read_to_string_limited(path, 8 * 1024 * 1024).map(Some)
}

/// 该配置文件里已存在的 MCP 名。
pub fn existing_names(text: Option<&str>, format: &McpFormat) -> Vec<String> {
    // Windows 编辑器常写 UTF-8 BOM，解析前必须去掉
    let Some(text) = text.map(crate::text::strip_bom) else {
        return Vec::new();
    };
    match format {
        McpFormat::Json { pointer } => merge_json::existing_names(text, pointer),
        McpFormat::Toml { table } => merge_toml::existing_names(text, table),
        McpFormat::Yaml { top_key } => merge_yaml::existing_names(text, top_key),
    }
}

/// 合并写入（纯函数）：返回新的文件全文。
///
/// 原文件带 UTF-8 BOM 时，解析前去掉、输出后补回——否则 JSON/TOML 解析会失败，
/// 用户还会看到「BOM 被悄悄删掉」的额外 diff。
pub fn upsert(text: Option<&str>, format: &McpFormat, servers: &[McpServer]) -> Result<String> {
    let had_bom = text.map(crate::text::has_bom).unwrap_or(false);
    let clean = text.map(crate::text::strip_bom);
    let merged = match format {
        McpFormat::Json { pointer } => merge_json::upsert(clean, pointer, servers),
        McpFormat::Toml { table } => merge_toml::upsert(clean, table, servers),
        McpFormat::Yaml { top_key } => merge_yaml::upsert(clean, top_key, servers),
    }?;
    Ok(crate::text::with_bom_if(had_bom, merged))
}

/// plan 阶段就把 env:/keychain: 引用解析成真值，失败即整体中止，避免写半套配置。
pub fn resolve_secrets(servers: &[McpServer]) -> Result<Vec<McpServer>> {
    let mut out = Vec::with_capacity(servers.len());
    for server in servers {
        let transport = match &server.transport {
            Transport::Stdio {
                command,
                args,
                env,
                cwd,
            } => {
                let mut resolved = BTreeMap::new();
                for (k, v) in env {
                    resolved.insert(k.clone(), literal(secrets::resolve(v)?));
                }
                Transport::Stdio {
                    command: command.clone(),
                    args: args.clone(),
                    env: resolved,
                    cwd: cwd.clone(),
                }
            }
            Transport::Http { url, headers } => {
                let mut resolved = BTreeMap::new();
                for (k, v) in headers {
                    resolved.insert(k.clone(), literal(secrets::resolve(v)?));
                }
                Transport::Http {
                    url: url.clone(),
                    headers: resolved,
                }
            }
        };
        out.push(McpServer {
            name: server.name.clone(),
            transport,
            enabled: server.enabled,
            timeout_secs: server.timeout_secs,
            note: server.note.clone(),
        });
    }
    Ok(out)
}

fn literal(value: String) -> SecretRef {
    SecretRef::Literal {
        value,
        acknowledged: true,
    }
}

/// 读取某个 agent 配置里已有的 MCP 定义（用于聚合视图与差异对比）。
pub fn read_servers(file: &McpFile) -> Vec<McpServer> {
    let Ok(Some(raw)) = read_text(&file.path) else {
        return Vec::new();
    };
    let text = crate::text::strip_bom(&raw);
    match &file.format {
        McpFormat::Json { pointer } => read_json(text, pointer),
        McpFormat::Toml { table } => read_toml(text, table),
        McpFormat::Yaml { top_key } => read_yaml(text, top_key),
    }
}

fn read_json(text: &str, pointer: &[String]) -> Vec<McpServer> {
    let Ok(doc) = serde_json::from_str::<Value>(text) else {
        return Vec::new();
    };
    let mut node = &doc;
    for key in pointer {
        match node.get(key) {
            Some(next) => node = next,
            None => return Vec::new(),
        }
    }
    let Some(map) = node.as_object() else {
        return Vec::new();
    };
    map.iter()
        .map(|(name, value)| server_from_json(name, value))
        .collect()
}

fn server_from_json(name: &str, value: &Value) -> McpServer {
    let str_vec = |key: &str| -> Vec<String> {
        value
            .get(key)
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default()
    };
    let secret_map = |key: &str| -> BTreeMap<String, SecretRef> {
        value
            .get(key)
            .and_then(|v| v.as_object())
            .map(|m| {
                m.iter()
                    .map(|(k, v)| {
                        (
                            k.clone(),
                            SecretRef::Literal {
                                value: v.as_str().unwrap_or_default().to_string(),
                                acknowledged: false,
                            },
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    };

    let transport = match value.get("command").and_then(|v| v.as_str()) {
        Some(command) => Transport::Stdio {
            command: command.to_string(),
            args: str_vec("args"),
            env: secret_map("env"),
            cwd: value
                .get("cwd")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
        },
        None => Transport::Http {
            url: value
                .get("url")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            headers: secret_map("headers"),
        },
    };
    McpServer {
        name: name.to_string(),
        transport,
        enabled: value
            .get("enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
        timeout_secs: None,
        note: None,
    }
}

fn read_toml(text: &str, table: &str) -> Vec<McpServer> {
    let Ok(doc) = text.parse::<DocumentMut>() else {
        return Vec::new();
    };
    let Some(tbl) = doc.get(table).and_then(|i| i.as_table_like()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (name, item) in tbl.iter() {
        let Some(inner) = item.as_table_like() else {
            continue;
        };
        let get_str = |key: &str| {
            inner
                .get(key)
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        };
        let args = inner
            .get("args")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        let transport = match get_str("command") {
            Some(command) => Transport::Stdio {
                command,
                args,
                env: BTreeMap::new(),
                cwd: get_str("cwd"),
            },
            None => Transport::Http {
                url: get_str("url").unwrap_or_default(),
                headers: BTreeMap::new(),
            },
        };
        out.push(McpServer {
            name: name.to_string(),
            transport,
            enabled: inner
                .get("enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(true),
            timeout_secs: inner
                .get("startup_timeout_sec")
                .and_then(|v| v.as_integer())
                .map(|v| v as u32),
            note: None,
        });
    }
    out
}

/// Hermes 的 YAML 只做轻量解析（名字 + 命令首行），不引入完整 YAML 数据模型。
fn read_yaml(text: &str, top_key: &str) -> Vec<McpServer> {
    let names = merge_yaml::existing_names(text, top_key);
    let lines: Vec<&str> = text.lines().collect();
    names
        .into_iter()
        .map(|name| {
            let command = lines
                .iter()
                .skip_while(|l| !l.trim_start().starts_with(&format!("{name}:")))
                .skip(1)
                .take_while(|l| l.starts_with("    ") || l.trim().is_empty())
                .find_map(|l| {
                    l.trim()
                        .strip_prefix("command:")
                        .map(|v| v.trim().trim_matches('"').to_string())
                })
                .unwrap_or_default();
            McpServer {
                name,
                transport: Transport::Stdio {
                    command,
                    args: Vec::new(),
                    env: BTreeMap::new(),
                    cwd: None,
                },
                enabled: true,
                timeout_secs: None,
                note: None,
            }
        })
        .collect()
}
