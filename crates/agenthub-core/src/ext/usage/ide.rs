//! CodeBuddy IDE 的用量解析。
//!
//! IDE 不写 JSONL：用量在
//! `CodeBuddyExtension/Data/**/CodeBuddyIDE/**/history/<conv>/<req>/index.json`
//! 的 `requests[].usage` 里。同目录树还有一份 `history/<conv>/index.json`（会话清单），
//! **不含用量但带模型**（`conversations[].selectedModelId`），两者同名、层级差一层，
//! 只按「上三级是 history」判定明细，避免把清单当明细扫。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::platform;
use crate::text;

use super::jsonl::{Usage, fingerprint, hour_of, secs_from_number, usage_of};
use super::{ParsedFile, ParsedRow, UNKNOWN_MODEL_ID};

/// 明细文件：`.../history/<conv>/<req>/index.json`。
pub fn is_detail_index(path: &Path) -> bool {
    path.file_name().and_then(|n| n.to_str()) == Some("index.json")
        && path
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            == Some("history")
}

/// 明细文件所属的会话 id（即 `history/<conv>` 的目录名）。
pub fn conversation_id(path: &Path) -> Option<String> {
    path.parent()?
        .parent()?
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .filter(|name| !name.is_empty())
}

/// 为明细文件找模型：读同级上一层的会话清单，按会话 id 匹配 `conversations[].selectedModelId`。
///
/// 一个清单文件可能列出多个会话，优先精确匹配 id，匹配不到才退回第一条（IDE 通常
/// 一个目录一个会话，退路只在数据不完整时生效）。
pub fn load_models(files: &[PathBuf]) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for path in files {
        let Some(conv) = conversation_id(path) else {
            continue;
        };
        if out.contains_key(&conv) {
            continue;
        }
        let Some(list_path) = path
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.join("index.json"))
        else {
            continue;
        };
        let Ok(text) = std::fs::read_to_string(platform::long_path(&list_path)) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<Value>(text::strip_bom(&text)) else {
            continue;
        };
        let Some(items) = value.get("conversations").and_then(Value::as_array) else {
            continue;
        };
        let model = items
            .iter()
            .find(|item| item.get("id").and_then(Value::as_str) == Some(conv.as_str()))
            .or_else(|| items.first())
            .and_then(|item| item.get("selectedModelId"))
            .and_then(Value::as_str)
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty());
        if let Some(model) = model {
            out.insert(conv, model);
        }
    }
    out
}

fn timestamp_of(value: &Value) -> Option<i64> {
    let raw = value.get("startedAt").and_then(|v| v.as_i64()).or_else(|| {
        value
            .get("startedAt")
            .and_then(|v| v.as_u64())
            .map(|v| v as i64)
    });
    raw.map(secs_from_number)
}

/// 解析一个明细文件。`model` 由调用方从会话清单带进来（请求里没有模型字段）。
pub fn parse_detail(path: &Path, model: &str) -> ParsedFile {
    let mut out = ParsedFile::default();
    let Ok(text) = std::fs::read_to_string(platform::long_path(path)) else {
        out.bad_lines = 1;
        return out;
    };
    // 记事本 / PowerShell 5.1 会写 BOM，`serde_json` 直接报错
    let Ok(value) = serde_json::from_str::<Value>(text::strip_bom(&text)) else {
        out.bad_lines = 1;
        return out;
    };
    let Some(requests) = value.get("requests").and_then(Value::as_array) else {
        // 这是会话清单，不含用量
        return out;
    };
    let model = if model.trim().is_empty() {
        UNKNOWN_MODEL_ID.to_string()
    } else {
        model.trim().to_string()
    };
    for request in requests {
        let Some(usage) = request.get("usage").and_then(usage_of) else {
            continue;
        };
        let Some(ts) = timestamp_of(request) else {
            out.bad_lines += 1;
            continue;
        };
        push_row(&mut out, ts, &model, usage);
    }
    out
}

fn push_row(out: &mut ParsedFile, ts: i64, model: &str, usage: Usage) {
    if usage.input == 0 && usage.output == 0 && usage.cache_read == 0 && usage.cache_write == 0 {
        return;
    }
    out.rows.push(ParsedRow {
        hour_utc: hour_of(ts),
        model: model.to_string(),
        provider: None,
        input: usage.input,
        cache_read: usage.cache_read,
        cache_write: usage.cache_write,
        output: usage.output,
        fingerprint: fingerprint(ts, model, &usage),
    });
}
