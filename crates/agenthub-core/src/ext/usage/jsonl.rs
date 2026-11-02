//! 三种 JSONL 会话日志的解析：Claude Code、Codex、OpenAI 风格（CodeBuddy CLI / WorkBuddy）。
//!
//! 每个文件按行流式读取，先用 `contains` 做行级预过滤再反序列化——这些文件里绝大多数
//! 字节是对话正文（`content` / `response_item`），全量解析会把内存打爆。

use std::collections::HashSet;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader};
use std::path::Path;

use serde::Deserialize;
use serde_json::Value;

use crate::platform;

use super::{ParsedFile, ParsedRow, UNKNOWN_MODEL_ID};

/// 把秒级时间戳落到整点（UTC）。桶键统一用 UTC，展示时再转本地时区。
pub(super) fn hour_of(epoch_secs: i64) -> i64 {
    epoch_secs.div_euclid(3600) * 3600
}

/// 按内容生成指纹（OpenAI 风格日志用）。
///
/// `DefaultHasher::new()` 的 key 是固定的，跨进程重跑也稳定，可以落进缓存复用。
pub(super) fn fingerprint(ts: i64, model: &str, usage: &Usage) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    ts.hash(&mut hasher);
    model.hash(&mut hasher);
    usage.input.hash(&mut hasher);
    usage.cache_read.hash(&mut hasher);
    usage.cache_write.hash(&mut hasher);
    usage.output.hash(&mut hasher);
    hasher.finish()
}

/// 归一后的单次调用用量（`input` 已是非缓存输入）。
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Usage {
    pub input: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub output: u64,
}

fn clamp(raw: i64) -> u64 {
    if raw > 0 { raw as u64 } else { 0 }
}

/// 按消息 id 生成指纹（Claude 用）。
fn id_fingerprint(id: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    id.hash(&mut hasher);
    hasher.finish()
}

fn reader(path: &Path) -> Option<BufReader<File>> {
    File::open(platform::long_path(path))
        .ok()
        .map(BufReader::new)
}

/// 去掉行首 BOM。
///
/// Windows 记事本 / PowerShell 5.1 的 `Set-Content` 会给文件加 BOM，落在第一行就是
/// `\u{feff}{`——`serde_json` 直接报「expected value at line 1 column 1」，整行被当成
/// 坏行丢掉（config.json 早就踩过同一个坑，见 `text::strip_bom`）。
fn clean(line: &str) -> &str {
    line.trim_start_matches('\u{feff}')
}

fn epoch_secs(raw: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(raw.trim())
        .ok()
        .map(|dt| dt.timestamp())
}

/// 毫秒或秒都接受：大于 1e11 视为毫秒（2025 年之后的毫秒时间戳都在这个量级）。
pub(super) fn secs_from_number(raw: i64) -> i64 {
    if raw.abs() > 100_000_000_000 {
        raw.div_euclid(1000)
    } else {
        raw
    }
}

fn number_of(value: Option<&Value>) -> Option<i64> {
    let value = value?;
    value
        .as_i64()
        .or_else(|| value.as_u64().map(|v| v.min(i64::MAX as u64) as i64))
        .or_else(|| value.as_f64().map(|v| v as i64))
        .or_else(|| value.as_str().and_then(|s| s.trim().parse::<i64>().ok()))
}

// ---------------------------------------------------------------- Claude Code

#[derive(Deserialize)]
struct ClaudeLine {
    #[serde(rename = "type", default)]
    kind: Option<String>,
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(rename = "requestedModel", default)]
    requested_model: Option<String>,
    #[serde(default)]
    message: Option<ClaudeMessage>,
}

#[derive(Deserialize)]
struct ClaudeMessage {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    usage: Option<AnthropicUsage>,
}

#[derive(Deserialize, Default)]
struct AnthropicUsage {
    #[serde(default)]
    input_tokens: i64,
    #[serde(default)]
    output_tokens: i64,
    #[serde(default)]
    cache_read_input_tokens: i64,
    #[serde(default)]
    cache_creation_input_tokens: i64,
}

/// Claude Code：`message.usage` 的 `input_tokens` **不含**缓存读写。
///
/// - 文件内按 `message.id` 去重：流式写入会重复追加同一条助手消息。
/// - `isSidechain` 的子代理消耗照计——那是真实发生的花费。
pub fn parse_claude(path: &Path) -> ParsedFile {
    let mut out = ParsedFile::default();
    let Some(reader) = reader(path) else {
        out.bad_lines = 1;
        return out;
    };
    let mut seen: HashSet<String> = HashSet::new();
    for line in reader.lines() {
        let Ok(line) = line else {
            out.bad_lines += 1;
            continue;
        };
        let line = clean(&line);
        if !line.contains("\"usage\"") {
            continue;
        }
        let Ok(record) = serde_json::from_str::<ClaudeLine>(line) else {
            out.bad_lines += 1;
            continue;
        };
        if record.kind.as_deref() != Some("assistant") {
            continue;
        }
        let Some(message) = record.message else {
            continue;
        };
        let Some(raw) = message.usage else {
            continue;
        };
        if let Some(id) = &message.id
            && !seen.insert(id.clone())
        {
            continue;
        }
        let Some(ts) = record.timestamp.as_deref().and_then(epoch_secs) else {
            // 没有时间戳就无法归到任何一天；记成坏行比猜一个日期诚实
            out.bad_lines += 1;
            continue;
        };
        // 子代理日志（`subagents/`）里的消息 id 不在父文件，但同一条消息仍可能被
        // 重放到多个文件；用 id 做跨文件指纹，文件内已由上面的 HashSet 兜底。
        let fingerprint = message.id.as_deref().map(id_fingerprint).unwrap_or(0);
        let usage = Usage {
            input: clamp(raw.input_tokens),
            cache_read: clamp(raw.cache_read_input_tokens),
            cache_write: clamp(raw.cache_creation_input_tokens),
            output: clamp(raw.output_tokens),
        };
        if usage.input == 0 && usage.output == 0 && usage.cache_read == 0 && usage.cache_write == 0
        {
            continue;
        }
        let model = message
            .model
            .filter(|m| !m.trim().is_empty())
            .or(record.requested_model)
            .unwrap_or_else(|| UNKNOWN_MODEL_ID.to_string());
        out.rows.push(ParsedRow {
            hour_utc: hour_of(ts),
            model,
            provider: None,
            input: usage.input,
            cache_read: usage.cache_read,
            cache_write: usage.cache_write,
            output: usage.output,
            fingerprint,
        });
    }
    out
}

// ---------------------------------------------------------------------- Codex

#[derive(Deserialize)]
struct CodexLine {
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(rename = "type", default)]
    kind: Option<String>,
    #[serde(default)]
    payload: Option<CodexPayload>,
}

#[derive(Deserialize, Default)]
struct CodexPayload {
    #[serde(rename = "type", default)]
    kind: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    model_provider: Option<String>,
    #[serde(default)]
    info: Option<CodexInfo>,
}

#[derive(Deserialize, Default)]
struct CodexInfo {
    #[serde(default)]
    total_token_usage: Option<CodexUsage>,
}

#[derive(Deserialize, Default, Clone, Copy)]
struct CodexUsage {
    #[serde(default)]
    input_tokens: i64,
    #[serde(default)]
    cached_input_tokens: i64,
    #[serde(default)]
    cache_write_input_tokens: i64,
    #[serde(default)]
    output_tokens: i64,
}

impl CodexUsage {
    fn delta_since(&self, prev: Option<CodexUsage>) -> CodexUsage {
        let Some(prev) = prev else {
            return *self;
        };
        // 累计值回退意味着会话重启 / 分支，当成新起点，否则会算成 0 而漏账
        if self.input_tokens < prev.input_tokens || self.output_tokens < prev.output_tokens {
            return *self;
        }
        CodexUsage {
            input_tokens: self.input_tokens - prev.input_tokens,
            cached_input_tokens: self.cached_input_tokens - prev.cached_input_tokens,
            cache_write_input_tokens: self.cache_write_input_tokens - prev.cache_write_input_tokens,
            output_tokens: self.output_tokens - prev.output_tokens,
        }
    }
}

/// Codex：`token_count` 事件带的是**累计值**，用相邻两次的差值计算增量。
///
/// 这比累加 `last_token_usage` 稳：中间丢事件也不会漏算。模型名不在该事件里，
/// 而是来自最近一次 `turn_context.payload.model`；provider 来自 `session_meta`。
pub fn parse_codex(path: &Path) -> ParsedFile {
    let mut out = ParsedFile::default();
    let Some(reader) = reader(path) else {
        out.bad_lines = 1;
        return out;
    };
    let mut model = UNKNOWN_MODEL_ID.to_string();
    let mut provider: Option<String> = None;
    let mut last_total: Option<CodexUsage> = None;
    for line in reader.lines() {
        let Ok(line) = line else {
            out.bad_lines += 1;
            continue;
        };
        let line = clean(&line);
        if !(line.contains("\"token_count\"")
            || line.contains("\"turn_context\"")
            || line.contains("\"session_meta\""))
        {
            continue;
        }
        let Ok(record) = serde_json::from_str::<CodexLine>(line) else {
            out.bad_lines += 1;
            continue;
        };
        let Some(payload) = record.payload else {
            continue;
        };
        match record.kind.as_deref() {
            Some("session_meta") => {
                if let Some(name) = payload.model_provider.filter(|p| !p.trim().is_empty()) {
                    provider = Some(name);
                }
            }
            Some("turn_context") => {
                if let Some(name) = payload.model.filter(|m| !m.trim().is_empty()) {
                    model = name;
                }
            }
            Some("event_msg") => {
                if payload.kind.as_deref() != Some("token_count") {
                    continue;
                }
                let Some(total) = payload.info.and_then(|info| info.total_token_usage) else {
                    continue;
                };
                let delta = total.delta_since(last_total);
                last_total = Some(total);
                let Some(ts) = record.timestamp.as_deref().and_then(epoch_secs) else {
                    out.bad_lines += 1;
                    continue;
                };
                let usage = Usage {
                    // Codex 的 input_tokens 已包含缓存命中
                    input: clamp(delta.input_tokens)
                        .saturating_sub(clamp(delta.cached_input_tokens)),
                    cache_read: clamp(delta.cached_input_tokens),
                    cache_write: clamp(delta.cache_write_input_tokens),
                    output: clamp(delta.output_tokens),
                };
                if usage.input == 0
                    && usage.output == 0
                    && usage.cache_read == 0
                    && usage.cache_write == 0
                {
                    continue;
                }
                out.rows.push(ParsedRow {
                    hour_utc: hour_of(ts),
                    model: model.clone(),
                    provider: provider.clone(),
                    input: usage.input,
                    cache_read: usage.cache_read,
                    cache_write: usage.cache_write,
                    output: usage.output,
                    fingerprint: 0,
                });
            }
            _ => {}
        }
    }
    out
}

// ------------------------------------------------- CodeBuddy CLI / WorkBuddy

fn num_field(object: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<i64> {
    keys.iter().find_map(|key| number_of(object.get(*key)))
}

fn cached_of(object: &serde_json::Map<String, Value>) -> i64 {
    if let Some(value) = num_field(
        object,
        &[
            "cache_read_input_tokens",
            "cacheReadInputTokens",
            "prompt_cache_hit_tokens",
            "cached_tokens",
            "cacheTokens",
        ],
    ) {
        return value;
    }
    for key in ["inputTokensDetails", "prompt_tokens_details"] {
        let Some(details) = object.get(key) else {
            continue;
        };
        if let Some(array) = details.as_array() {
            for item in array {
                if let Some(found) = item
                    .as_object()
                    .and_then(|inner| num_field(inner, &["cached_tokens"]))
                {
                    return found;
                }
            }
        } else if let Some(found) = details
            .as_object()
            .and_then(|inner| num_field(inner, &["cached_tokens"]))
        {
            return found;
        }
    }
    0
}

/// 识别一个 usage 对象。`input` 是必填锚点（允许为 0，但不允许缺失）。
pub(super) fn usage_of(value: &Value) -> Option<Usage> {
    let object = value.as_object()?;
    let input = num_field(object, &["input_tokens", "inputTokens", "prompt_tokens"])?;
    let output = num_field(
        object,
        &["output_tokens", "outputTokens", "completion_tokens"],
    )
    .unwrap_or(0);
    let cache_read = cached_of(object);
    let cache_write = num_field(
        object,
        &[
            "cache_write_input_tokens",
            "cacheWriteInputTokens",
            "cache_creation_input_tokens",
            "prompt_cache_write_tokens",
            "cachedWriteTokens",
        ],
    )
    .unwrap_or(0);
    Some(Usage {
        // 这一系的 input 已含缓存命中，未命中部分是差值
        input: clamp(input).saturating_sub(clamp(cache_read)),
        cache_read: clamp(cache_read),
        cache_write: clamp(cache_write),
        output: clamp(output),
    })
}

fn pick_usage(value: &Value) -> Option<Usage> {
    let candidates = [
        value.get("message").and_then(|m| m.get("usage")),
        value.get("providerData").and_then(|p| p.get("usage")),
        value.get("providerData").and_then(|p| p.get("rawUsage")),
        value.get("usage"),
    ];
    candidates.into_iter().flatten().find_map(usage_of)
}

fn model_of(value: &Value) -> Option<String> {
    let provider = value.get("providerData");
    provider
        .and_then(|p| p.get("model"))
        .and_then(Value::as_str)
        .or_else(|| {
            provider
                .and_then(|p| p.get("requestModelId"))
                .and_then(Value::as_str)
        })
        .or_else(|| {
            value
                .get("message")
                .and_then(|m| m.get("model"))
                .and_then(Value::as_str)
        })
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
}

/// OpenAI 风格 JSONL（CodeBuddy CLI / WorkBuddy）。
///
/// 一行里可能同时出现 `message.usage`（snake_case）与 `providerData.usage`（camelCase），
/// 只取第一个能识别的，避免同一请求被计两次。指纹用于跨文件去重：复制会话会重放历史。
pub fn parse_openai_jsonl(path: &Path) -> ParsedFile {
    let mut out = ParsedFile::default();
    let Some(reader) = reader(path) else {
        out.bad_lines = 1;
        return out;
    };
    for line in reader.lines() {
        let Ok(line) = line else {
            out.bad_lines += 1;
            continue;
        };
        let line = clean(&line);
        if !line.contains("\"usage\"") {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            out.bad_lines += 1;
            continue;
        };
        let Some(usage) = pick_usage(&value) else {
            continue;
        };
        let Some(ts) = number_of(value.get("timestamp")).map(secs_from_number) else {
            out.bad_lines += 1;
            continue;
        };
        let model = model_of(&value).unwrap_or_else(|| UNKNOWN_MODEL_ID.to_string());
        if usage.input == 0 && usage.output == 0 && usage.cache_read == 0 && usage.cache_write == 0
        {
            continue;
        }
        out.rows.push(ParsedRow {
            hour_utc: hour_of(ts),
            provider: None,
            fingerprint: fingerprint(ts, &model, &usage),
            model,
            input: usage.input,
            cache_read: usage.cache_read,
            cache_write: usage.cache_write,
            output: usage.output,
        });
    }
    out
}
