//! Chat Completions 上游 → Anthropic Messages 客户端。
//!
//! 即「Claude Code 接一个只提供 OpenAI 兼容接口的厂商」这条最常走的路。
//!
//! 两处硬要求：
//! - **工具调用的 arguments 是字符串片段**，必须原样转成 `input_json_delta`，
//!   不能先解析成对象再吐（片段本身就不是合法 JSON）；
//! - **流式必须增量**，攒完整条响应再转会让客户端等到超时。

use std::collections::{BTreeMap, BTreeSet};

use bytes::Bytes;
use serde_json::{Value, json};

use super::sse::{SseEvent, encode};

/// Anthropic 请求体 → Chat 请求体。`model` 由调用方按上游映射决定。
pub fn request(body: &Value, model: &str) -> Value {
    let mut out = serde_json::Map::new();
    out.insert("model".into(), json!(model));
    out.insert(
        "stream".into(),
        json!(body.get("stream").and_then(Value::as_bool).unwrap_or(false)),
    );

    let mut messages: Vec<Value> = Vec::new();
    if let Some(system) = body.get("system") {
        let text = text_of(system);
        if !text.is_empty() {
            messages.push(json!({ "role": "system", "content": text }));
        }
    }
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
        convert_message(role, message.get("content"), &mut messages);
    }
    out.insert("messages".into(), Value::Array(messages));

    if let Some(max_tokens) = body.get("max_tokens") {
        // Anthropic 必填、Chat 可选；沿用客户端值
        out.insert("max_tokens".into(), max_tokens.clone());
    }
    for (from, to) in [
        ("temperature", "temperature"),
        ("top_p", "top_p"),
        ("top_k", "top_k"),
    ] {
        if let Some(value) = body.get(from) {
            out.insert(to.into(), value.clone());
        }
    }
    if let Some(stops) = body.get("stop_sequences").and_then(Value::as_array)
        && !stops.is_empty()
    {
        out.insert("stop".into(), Value::Array(stops.clone()));
    }

    if let Some(tools) = body.get("tools").and_then(Value::as_array) {
        // Anthropic 的 server tools（web_search 之类）没有 input_schema，Chat 侧没法表达
        let mapped: Vec<Value> = tools
            .iter()
            .filter(|tool| tool.get("input_schema").is_some())
            .map(|tool| {
                json!({
                    "type": "function",
                    "function": {
                        "name": tool.get("name").cloned().unwrap_or(json!("")),
                        "description": tool.get("description").cloned().unwrap_or(json!("")),
                        "parameters": tool.get("input_schema").cloned().unwrap_or(json!({"type":"object"})),
                    }
                })
            })
            .collect();
        if !mapped.is_empty() {
            out.insert("tools".into(), Value::Array(mapped));
        }
    }

    if let Some(choice) = body.get("tool_choice") {
        let kind = choice.get("type").and_then(Value::as_str).unwrap_or("auto");
        let mapped = match kind {
            "any" => json!("required"),
            "tool" => json!({
                "type": "function",
                "function": { "name": choice.get("name").cloned().unwrap_or(json!("")) }
            }),
            "none" => json!("none"),
            _ => json!("auto"),
        };
        out.insert("tool_choice".into(), mapped);
    }

    Value::Object(out)
}

fn convert_message(role: &str, content: Option<&Value>, out: &mut Vec<Value>) {
    let Some(content) = content else { return };
    if let Some(text) = content.as_str() {
        out.push(json!({ "role": role, "content": text }));
        return;
    }
    let Some(blocks) = content.as_array() else {
        return;
    };

    // Chat 的 `tool` 消息承接上一条 assistant 的 tool_calls，必须单独成条
    for block in blocks {
        if block.get("type").and_then(Value::as_str) != Some("tool_result") {
            continue;
        }
        out.push(json!({
            "role": "tool",
            "tool_call_id": block.get("tool_use_id").cloned().unwrap_or(json!("")),
            "content": text_of(block.get("content").unwrap_or(&json!(""))),
        }));
    }

    let mut texts: Vec<String> = Vec::new();
    let mut images: Vec<Value> = Vec::new();
    let mut tool_calls: Vec<Value> = Vec::new();
    for block in blocks {
        match block.get("type").and_then(Value::as_str) {
            Some("text") => {
                if let Some(text) = block.get("text").and_then(Value::as_str) {
                    texts.push(text.to_string());
                }
            }
            Some("image") => images.push(image_part(block)),
            Some("tool_use") => tool_calls.push(json!({
                "id": block.get("id").cloned().unwrap_or(json!("")),
                "type": "function",
                "function": {
                    "name": block.get("name").cloned().unwrap_or(json!("")),
                    "arguments": serde_json::to_string(block.get("input").unwrap_or(&json!({})))
                        .unwrap_or_else(|_| "{}".into()),
                }
            })),
            // thinking 块在 Chat 侧没有对应形态，丢弃（不伪造）
            _ => {}
        }
    }

    let joined = texts.join("\n");
    if role == "assistant" {
        if joined.is_empty() && tool_calls.is_empty() {
            return;
        }
        let mut message = json!({ "role": "assistant" });
        // 只有图片时用 parts 数组；纯文本保持字符串（更多兼容层认这种）
        if images.is_empty() {
            message["content"] = json!(joined);
        } else {
            let mut parts = vec![json!({ "type": "text", "text": joined })];
            parts.extend(images);
            message["content"] = Value::Array(parts);
        }
        if !tool_calls.is_empty() {
            message["tool_calls"] = Value::Array(tool_calls);
        }
        out.push(message);
    } else if !joined.is_empty() || !images.is_empty() {
        if images.is_empty() {
            out.push(json!({ "role": "user", "content": joined }));
        } else {
            let mut parts = Vec::new();
            if !joined.is_empty() {
                parts.push(json!({ "type": "text", "text": joined }));
            }
            parts.extend(images);
            out.push(json!({ "role": "user", "content": parts }));
        }
    }
}

fn image_part(block: &Value) -> Value {
    let source = block.get("source").cloned().unwrap_or(json!({}));
    let url = match source.get("type").and_then(Value::as_str) {
        Some("base64") => {
            let media = source
                .get("media_type")
                .and_then(Value::as_str)
                .unwrap_or("image/png");
            let data = source.get("data").and_then(Value::as_str).unwrap_or("");
            format!("data:{media};base64,{data}")
        }
        _ => source
            .get("url")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
    };
    json!({ "type": "image_url", "image_url": { "url": url } })
}

/// Anthropic 的 `content` 既可以是字符串，也可以是 `[{type:text,text}]`。
fn text_of(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
            .filter_map(|block| block.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// Chat 非流式响应 → Anthropic message。
pub fn response(body: &Value) -> Value {
    let choice = body
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .cloned()
        .unwrap_or(json!({}));
    let message = choice.get("message").cloned().unwrap_or(json!({}));

    let mut content: Vec<Value> = Vec::new();
    if let Some(text) = message.get("content").and_then(Value::as_str)
        && !text.is_empty()
    {
        content.push(json!({ "type": "text", "text": text }));
    }
    for call in message
        .get("tool_calls")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        content.push(json!({
            "type": "tool_use",
            "id": call.get("id").cloned().unwrap_or(json!("")),
            "name": call
                .get("function")
                .and_then(|f| f.get("name"))
                .cloned()
                .unwrap_or(json!("")),
            "input": parse_arguments(
                call.get("function").and_then(|f| f.get("arguments"))
            ),
        }));
    }

    json!({
        "id": body.get("id").cloned().unwrap_or(json!("msg_agenthub")),
        "type": "message",
        "role": "assistant",
        "model": body.get("model").cloned().unwrap_or(json!("")),
        "content": Value::Array(content),
        "stop_reason": stop_reason(choice.get("finish_reason")),
        "stop_sequence": Value::Null,
        "usage": {
            "input_tokens": usage(body, "prompt_tokens"),
            "output_tokens": usage(body, "completion_tokens"),
        }
    })
}

fn usage(body: &Value, key: &str) -> u64 {
    body.get("usage")
        .and_then(|usage| usage.get(key))
        .and_then(Value::as_u64)
        .unwrap_or(0)
}

/// `arguments` 是 JSON 字符串；解析不了就给空对象（伪造内容比给空更危险）。
pub fn parse_arguments(value: Option<&Value>) -> Value {
    let Some(value) = value else {
        return json!({});
    };
    if value.is_object() {
        return value.clone();
    }
    let Some(text) = value.as_str() else {
        return json!({});
    };
    if text.trim().is_empty() {
        return json!({});
    }
    serde_json::from_str(text).unwrap_or_else(|_| json!({}))
}

pub fn stop_reason(finish_reason: Option<&Value>) -> Value {
    match finish_reason.and_then(Value::as_str) {
        Some("stop") => json!("end_turn"),
        Some("length") => json!("max_tokens"),
        Some("tool_calls") | Some("function_call") => json!("tool_use"),
        Some("content_filter") => json!("end_turn"),
        _ => Value::Null,
    }
}

/// Chat 流 → Anthropic 事件流。
#[derive(Default)]
pub struct Stream {
    started: bool,
    next_index: u64,
    text_index: Option<u64>,
    /// Chat 的 `tool_calls[].index` → Anthropic 的 content block index
    tool_blocks: BTreeMap<u64, u64>,
    open_tools: BTreeSet<u64>,
    stop_reason: Option<Value>,
    input_tokens: u64,
    output_tokens: u64,
    model: String,
    id: String,
}

impl Stream {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, event: &SseEvent) -> Vec<Bytes> {
        if event.is_done() {
            return Vec::new();
        }
        let Ok(chunk) = serde_json::from_str::<Value>(&event.data) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        if !self.started {
            self.started = true;
            if let Some(model) = chunk.get("model").and_then(Value::as_str) {
                self.model = model.to_string();
            }
            if let Some(id) = chunk.get("id").and_then(Value::as_str) {
                self.id = id.to_string();
            }
            self.input_tokens = usage(&chunk, "prompt_tokens");
            out.push(encode(
                Some("message_start"),
                &json!({
                    "type": "message_start",
                    "message": {
                        "id": if self.id.is_empty() { "msg_agenthub" } else { &self.id },
                        "type": "message",
                        "role": "assistant",
                        "model": self.model,
                        "content": [],
                        "stop_reason": Value::Null,
                        "stop_sequence": Value::Null,
                        "usage": { "input_tokens": self.input_tokens, "output_tokens": 0 }
                    }
                }),
            ));
        }

        let choice = chunk
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first())
            .cloned()
            .unwrap_or(json!({}));
        let delta = choice.get("delta").cloned().unwrap_or(json!({}));

        if let Some(text) = delta.get("content").and_then(Value::as_str)
            && !text.is_empty()
        {
            if self.text_index.is_none() {
                let index = self.alloc_index();
                self.text_index = Some(index);
                out.push(content_block_start(index, json!({"type":"text","text":""})));
            }
            let index = self.text_index.unwrap_or(0);
            out.push(content_block_delta(
                index,
                json!({ "type": "text_delta", "text": text }),
            ));
        }

        if let Some(calls) = delta.get("tool_calls").and_then(Value::as_array) {
            // 文本块必须收尾才能开工具块（Anthropic 的块是顺序的）
            if let Some(index) = self.text_index.take() {
                out.push(content_block_stop(index));
            }
            for call in calls {
                let chat_index = call.get("index").and_then(Value::as_u64).unwrap_or(0);
                let index = match self.tool_blocks.get(&chat_index) {
                    Some(index) => *index,
                    None => {
                        let index = self.alloc_index();
                        self.tool_blocks.insert(chat_index, index);
                        self.open_tools.insert(chat_index);
                        out.push(content_block_start(
                            index,
                            json!({
                                "type": "tool_use",
                                "id": call.get("id").cloned().unwrap_or(json!("")),
                                "name": call
                                    .get("function")
                                    .and_then(|f| f.get("name"))
                                    .cloned()
                                    .unwrap_or(json!("")),
                                "input": {},
                            }),
                        ));
                        index
                    }
                };
                if let Some(arguments) = call
                    .get("function")
                    .and_then(|f| f.get("arguments"))
                    .and_then(Value::as_str)
                    && !arguments.is_empty()
                {
                    out.push(content_block_delta(
                        index,
                        json!({ "type": "input_json_delta", "partial_json": arguments }),
                    ));
                }
            }
        }

        if let Some(reason) = choice.get("finish_reason")
            && !reason.is_null()
        {
            self.stop_reason = Some(stop_reason(Some(reason)));
        }
        let completion = usage(&chunk, "completion_tokens");
        if completion > 0 {
            self.output_tokens = completion;
        }

        out
    }

    /// 上游结束：关块 + message_delta + message_stop。少一条客户端就会一直等。
    pub fn finish(&mut self) -> Vec<Bytes> {
        let mut out = Vec::new();
        if let Some(index) = self.text_index.take() {
            out.push(content_block_stop(index));
        }
        let mut tool_indexes: Vec<u64> = self.open_tools.iter().copied().collect();
        tool_indexes.sort_unstable();
        for chat_index in tool_indexes {
            if let Some(index) = self.tool_blocks.get(&chat_index).copied() {
                out.push(content_block_stop(index));
            }
        }
        self.open_tools.clear();

        if self.started {
            out.push(encode(
                Some("message_delta"),
                &json!({
                    "type": "message_delta",
                    "delta": { "stop_reason": self.stop_reason.clone().unwrap_or(json!("end_turn")), "stop_sequence": Value::Null },
                    "usage": { "output_tokens": self.output_tokens }
                }),
            ));
            out.push(encode(
                Some("message_stop"),
                &json!({ "type": "message_stop" }),
            ));
        }
        out
    }

    fn alloc_index(&mut self) -> u64 {
        let index = self.next_index;
        self.next_index += 1;
        index
    }
}

fn content_block_start(index: u64, block: Value) -> Bytes {
    encode(
        Some("content_block_start"),
        &json!({ "type": "content_block_start", "index": index, "content_block": block }),
    )
}

fn content_block_delta(index: u64, delta: Value) -> Bytes {
    encode(
        Some("content_block_delta"),
        &json!({ "type": "content_block_delta", "index": index, "delta": delta }),
    )
}

fn content_block_stop(index: u64) -> Bytes {
    encode(
        Some("content_block_stop"),
        &json!({ "type": "content_block_stop", "index": index }),
    )
}
