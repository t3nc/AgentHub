//! Anthropic Messages 上游 → Chat Completions 客户端（反向转换）。
//!
//! 用得比正向少，但同理：客户端只认 Chat，上游只有 Anthropic 兼容接口时必须有它。

use std::collections::BTreeMap;

use bytes::Bytes;
use serde_json::{Value, json};

use super::sse::{DONE, SseEvent, encode};

/// Chat 请求体 → Anthropic 请求体。
pub fn request(body: &Value, model: &str) -> Value {
    let mut out = serde_json::Map::new();
    out.insert("model".into(), json!(model));
    out.insert(
        "stream".into(),
        json!(body.get("stream").and_then(Value::as_bool).unwrap_or(false)),
    );

    let mut system: Vec<String> = Vec::new();
    let mut messages: Vec<Value> = Vec::new();
    let mut pending_tool_results: Vec<Value> = Vec::new();

    for message in body
        .get("messages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let role = message
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or("user");
        match role {
            "system" => {
                let text = plain_text(message.get("content"));
                if !text.is_empty() {
                    system.push(text);
                }
            }
            "tool" => {
                // 连续的 tool 结果并进同一条 user 消息
                pending_tool_results.push(json!({
                    "type": "tool_result",
                    "tool_use_id": message.get("tool_call_id").cloned().unwrap_or(json!("")),
                    "content": plain_text(message.get("content")),
                }));
            }
            "assistant" => {
                flush_tool_results(&mut pending_tool_results, &mut messages);
                let mut blocks: Vec<Value> = Vec::new();
                let text = plain_text(message.get("content"));
                if !text.is_empty() {
                    blocks.push(json!({ "type": "text", "text": text }));
                }
                for call in message
                    .get("tool_calls")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    blocks.push(json!({
                        "type": "tool_use",
                        "id": call.get("id").cloned().unwrap_or(json!("")),
                        "name": call.get("function").and_then(|f| f.get("name")).cloned().unwrap_or(json!("")),
                        "input": super::chat_to_anthropic::parse_arguments(
                            call.get("function").and_then(|f| f.get("arguments"))
                        ),
                    }));
                }
                if !blocks.is_empty() {
                    messages.push(json!({ "role": "assistant", "content": blocks }));
                }
            }
            _ => {
                flush_tool_results(&mut pending_tool_results, &mut messages);
                let content = chat_content_to_anthropic(message.get("content"));
                if !content.is_empty() {
                    messages.push(json!({ "role": "user", "content": content }));
                }
            }
        }
    }
    flush_tool_results(&mut pending_tool_results, &mut messages);

    if !system.is_empty() {
        out.insert("system".into(), json!(system.join("\n")));
    }
    out.insert("messages".into(), Value::Array(messages));

    // Anthropic 的 max_tokens 是必填项；Chat 侧可以不给，这里给一个保守默认
    out.insert(
        "max_tokens".into(),
        body.get("max_tokens").cloned().unwrap_or(json!(4096)),
    );
    if let Some(temperature) = body.get("temperature") {
        out.insert("temperature".into(), temperature.clone());
    }
    if let Some(top_p) = body.get("top_p") {
        out.insert("top_p".into(), top_p.clone());
    }
    if let Some(stop) = body.get("stop") {
        let stops = match stop {
            Value::String(text) => vec![json!(text)],
            Value::Array(items) => items.clone(),
            _ => Vec::new(),
        };
        if !stops.is_empty() {
            out.insert("stop_sequences".into(), Value::Array(stops));
        }
    }

    if let Some(tools) = body.get("tools").and_then(Value::as_array) {
        let mapped: Vec<Value> = tools
            .iter()
            .filter_map(|tool| tool.get("function"))
            .map(|function| {
                json!({
                    "name": function.get("name").cloned().unwrap_or(json!("")),
                    "description": function.get("description").cloned().unwrap_or(json!("")),
                    "input_schema": function.get("parameters").cloned().unwrap_or(json!({"type":"object"})),
                })
            })
            .collect();
        if !mapped.is_empty() {
            out.insert("tools".into(), Value::Array(mapped));
        }
    }

    match body.get("tool_choice") {
        Some(Value::String(kind)) => {
            let mapped = match kind.as_str() {
                "required" => json!({ "type": "any" }),
                "none" => json!({ "type": "none" }),
                _ => json!({ "type": "auto" }),
            };
            out.insert("tool_choice".into(), mapped);
        }
        Some(Value::Object(object)) => {
            if let Some(name) = object
                .get("function")
                .and_then(|f| f.get("name"))
                .and_then(Value::as_str)
            {
                out.insert(
                    "tool_choice".into(),
                    json!({ "type": "tool", "name": name }),
                );
            }
        }
        _ => {}
    }

    Value::Object(out)
}

fn flush_tool_results(pending: &mut Vec<Value>, messages: &mut Vec<Value>) {
    if pending.is_empty() {
        return;
    }
    let blocks = std::mem::take(pending);
    messages.push(json!({ "role": "user", "content": blocks }));
}

/// Chat 的 content 是字符串，或 `[{type:text},{type:image_url}]` 数组。
fn chat_content_to_anthropic(content: Option<&Value>) -> Vec<Value> {
    let Some(content) = content else {
        return Vec::new();
    };
    match content {
        Value::String(text) => {
            if text.is_empty() {
                Vec::new()
            } else {
                vec![json!({ "type": "text", "text": text })]
            }
        }
        Value::Array(parts) => parts
            .iter()
            .filter_map(|part| match part.get("type").and_then(Value::as_str) {
                Some("text") => Some(json!({
                    "type": "text",
                    "text": part.get("text").cloned().unwrap_or(json!(""))
                })),
                Some("image_url") => {
                    let url = part
                        .get("image_url")
                        .and_then(|value| value.get("url"))
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    Some(data_uri_to_block(url))
                }
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn data_uri_to_block(url: &str) -> Value {
    if let Some(rest) = url.strip_prefix("data:")
        && let Some((meta, data)) = rest.split_once(',')
    {
        let media = meta.trim_end_matches(";base64");
        return json!({
            "type": "image",
            "source": { "type": "base64", "media_type": media, "data": data }
        });
    }
    json!({ "type": "image", "source": { "type": "url", "url": url } })
}

fn plain_text(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(parts)) => parts
            .iter()
            .filter_map(|part| part.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// Anthropic message → Chat completion。
pub fn response(body: &Value) -> Value {
    let mut text = String::new();
    let mut tool_calls: Vec<Value> = Vec::new();
    for block in body
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        match block.get("type").and_then(Value::as_str) {
            Some("text") => {
                if let Some(chunk) = block.get("text").and_then(Value::as_str) {
                    text.push_str(chunk);
                }
            }
            Some("tool_use") => tool_calls.push(json!({
                "id": block.get("id").cloned().unwrap_or(json!("")),
                "type": "function",
                "function": {
                    "name": block.get("name").cloned().unwrap_or(json!("")),
                    "arguments": serde_json::to_string(block.get("input").unwrap_or(&json!({})))
                        .unwrap_or_else(|_| "{}".into()),
                }
            })),
            _ => {}
        }
    }

    let mut message = json!({ "role": "assistant", "content": text });
    if !tool_calls.is_empty() {
        message["tool_calls"] = Value::Array(tool_calls);
    }

    json!({
        "id": body.get("id").cloned().unwrap_or(json!("chatcmpl-agenthub")),
        "object": "chat.completion",
        "created": 0,
        "model": body.get("model").cloned().unwrap_or(json!("")),
        "choices": [{
            "index": 0,
            "message": message,
            "finish_reason": finish_reason(body.get("stop_reason")),
        }],
        "usage": {
            "prompt_tokens": usage(body, "input_tokens"),
            "completion_tokens": usage(body, "output_tokens"),
            "total_tokens": usage(body, "input_tokens") + usage(body, "output_tokens"),
        }
    })
}

fn usage(body: &Value, key: &str) -> u64 {
    body.get("usage")
        .and_then(|usage| usage.get(key))
        .and_then(Value::as_u64)
        .unwrap_or(0)
}

pub fn finish_reason(stop_reason: Option<&Value>) -> Value {
    match stop_reason.and_then(Value::as_str) {
        Some("end_turn") | Some("stop_sequence") => json!("stop"),
        Some("max_tokens") => json!("length"),
        Some("tool_use") => json!("tool_calls"),
        _ => Value::Null,
    }
}

/// Anthropic 事件流 → Chat chunk 流。
#[derive(Default)]
pub struct Stream {
    started: bool,
    id: String,
    model: String,
    /// Anthropic content block index → Chat tool_calls index
    tool_map: BTreeMap<u64, u64>,
    next_tool: u64,
    finish: Option<Value>,
    input_tokens: u64,
    output_tokens: u64,
}

impl Stream {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, event: &SseEvent) -> Vec<Bytes> {
        let Ok(payload) = serde_json::from_str::<Value>(&event.data) else {
            return Vec::new();
        };
        let kind = payload
            .get("type")
            .and_then(Value::as_str)
            .or(event.event.as_deref())
            .unwrap_or("");
        let mut out = Vec::new();

        match kind {
            "message_start" => {
                let message = payload.get("message").cloned().unwrap_or(json!({}));
                if let Some(id) = message.get("id").and_then(Value::as_str) {
                    self.id = id.to_string();
                }
                if let Some(model) = message.get("model").and_then(Value::as_str) {
                    self.model = model.to_string();
                }
                self.input_tokens = message
                    .get("usage")
                    .and_then(|u| u.get("input_tokens"))
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                self.started = true;
                out.push(self.chunk(json!({ "role": "assistant", "content": "" }), Value::Null));
            }
            "content_block_start" => {
                let index = payload.get("index").and_then(Value::as_u64).unwrap_or(0);
                let block = payload.get("content_block").cloned().unwrap_or(json!({}));
                if block.get("type").and_then(Value::as_str) == Some("tool_use") {
                    let tool_index = self.next_tool;
                    self.next_tool += 1;
                    self.tool_map.insert(index, tool_index);
                    out.push(self.chunk(
                        json!({
                            "tool_calls": [{
                                "index": tool_index,
                                "id": block.get("id").cloned().unwrap_or(json!("")),
                                "type": "function",
                                "function": { "name": block.get("name").cloned().unwrap_or(json!("")), "arguments": "" }
                            }]
                        }),
                        Value::Null,
                    ));
                }
            }
            "content_block_delta" => {
                let index = payload.get("index").and_then(Value::as_u64).unwrap_or(0);
                let delta = payload.get("delta").cloned().unwrap_or(json!({}));
                match delta.get("type").and_then(Value::as_str) {
                    Some("text_delta") => {
                        let text = delta.get("text").and_then(Value::as_str).unwrap_or("");
                        if !text.is_empty() {
                            out.push(self.chunk(json!({ "content": text }), Value::Null));
                        }
                    }
                    Some("input_json_delta") => {
                        let partial = delta
                            .get("partial_json")
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        if let Some(tool_index) = self.tool_map.get(&index).copied()
                            && !partial.is_empty()
                        {
                            out.push(self.chunk(
                                json!({
                                    "tool_calls": [{
                                        "index": tool_index,
                                        "function": { "arguments": partial }
                                    }]
                                }),
                                Value::Null,
                            ));
                        }
                    }
                    _ => {}
                }
            }
            "message_delta" => {
                if let Some(reason) = payload.get("delta").and_then(|d| d.get("stop_reason")) {
                    self.finish = Some(finish_reason(Some(reason)));
                }
                if let Some(tokens) = payload
                    .get("usage")
                    .and_then(|u| u.get("output_tokens"))
                    .and_then(Value::as_u64)
                {
                    self.output_tokens = tokens;
                }
            }
            _ => {}
        }
        out
    }

    /// 收尾：一条带 finish_reason 的空 delta + `[DONE]`。
    pub fn finish(&mut self) -> Vec<Bytes> {
        if !self.started {
            return Vec::new();
        }
        let mut out = vec![self.chunk(json!({}), self.finish.clone().unwrap_or(json!("stop")))];
        out.push(Bytes::from_static(DONE.as_bytes()));
        out
    }

    fn chunk(&self, delta: Value, finish_reason: Value) -> Bytes {
        encode(
            None,
            &json!({
                "id": if self.id.is_empty() { "chatcmpl-agenthub" } else { &self.id },
                "object": "chat.completion.chunk",
                "created": 0,
                "model": self.model,
                "choices": [{ "index": 0, "delta": delta, "finish_reason": finish_reason }],
                "usage": if finish_reason.is_null() { Value::Null } else {
                    json!({
                        "prompt_tokens": self.input_tokens,
                        "completion_tokens": self.output_tokens,
                        "total_tokens": self.input_tokens + self.output_tokens,
                    })
                },
            }),
        )
    }
}
