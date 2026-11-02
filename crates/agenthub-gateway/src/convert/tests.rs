//! 转换器的离线用例：不连网，只比对「输入什么 → 输出什么」。

use super::*;
use serde_json::json;

fn collect(converter: &mut StreamConverter, decoder: &mut SseDecoder, raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    for event in decoder.push(raw.as_bytes()) {
        for bytes in converter.push(&event) {
            out.push(String::from_utf8_lossy(&bytes).to_string());
        }
    }
    out
}

fn drain(converter: &mut StreamConverter, decoder: &mut SseDecoder) -> Vec<String> {
    let mut out = Vec::new();
    for event in decoder.finish() {
        for bytes in converter.push(&event) {
            out.push(String::from_utf8_lossy(&bytes).to_string());
        }
    }
    for bytes in converter.finish() {
        out.push(String::from_utf8_lossy(&bytes).to_string());
    }
    out
}

#[test]
fn direction_only_covers_anthropic_and_chat() {
    use GatewayProtocol::{Anthropic, ChatCompletions, Responses};
    assert_eq!(
        direction(Anthropic, ChatCompletions),
        Some(Direction::AnthropicFromChat)
    );
    assert_eq!(
        direction(ChatCompletions, Anthropic),
        Some(Direction::ChatFromAnthropic)
    );
    assert_eq!(direction(Anthropic, Anthropic), None, "同协议不转换");
    assert_eq!(
        direction(Responses, Anthropic),
        None,
        "Responses 不参与转换"
    );
    assert_eq!(
        upstream_path(Direction::AnthropicFromChat),
        "/v1/chat/completions"
    );
    assert_eq!(upstream_path(Direction::ChatFromAnthropic), "/v1/messages");
}

#[test]
fn anthropic_request_becomes_chat() {
    let body = json!({
        "model": "claude-sonnet-4-5",
        "max_tokens": 1024,
        "stream": true,
        "system": [{ "type": "text", "text": "be brief" }],
        "messages": [
            { "role": "user", "content": "hi" },
            { "role": "assistant", "content": [
                { "type": "text", "text": "let me read" },
                { "type": "tool_use", "id": "t1", "name": "Read", "input": { "path": "a" } }
            ]},
            { "role": "user", "content": [
                { "type": "tool_result", "tool_use_id": "t1", "content": "file body" },
                { "type": "text", "text": "continue" }
            ]}
        ],
        "tools": [{ "name": "Read", "description": "read", "input_schema": { "type": "object" } }],
        "tool_choice": { "type": "auto" }
    });

    let out = convert_request(Direction::AnthropicFromChat, &body, "deepseek-chat");
    assert_eq!(out["model"], "deepseek-chat");
    assert_eq!(out["stream"], true);
    assert_eq!(out["max_tokens"], 1024);
    assert_eq!(out["messages"][0]["role"], "system");
    assert_eq!(out["messages"][0]["content"], "be brief");

    let assistant = &out["messages"][2];
    assert_eq!(assistant["role"], "assistant");
    assert_eq!(assistant["content"], "let me read");
    assert_eq!(assistant["tool_calls"][0]["id"], "t1");
    assert_eq!(assistant["tool_calls"][0]["function"]["name"], "Read");
    assert_eq!(
        assistant["tool_calls"][0]["function"]["arguments"], "{\"path\":\"a\"}",
        "arguments 必须是 JSON 字符串"
    );

    // tool 结果要单独成一条 role:tool，且排在文本之前
    assert_eq!(out["messages"][3]["role"], "tool");
    assert_eq!(out["messages"][3]["tool_call_id"], "t1");
    assert_eq!(out["messages"][3]["content"], "file body");
    assert_eq!(out["messages"][4]["content"], "continue");

    assert_eq!(out["tools"][0]["function"]["parameters"]["type"], "object");
    assert_eq!(out["tool_choice"], "auto");
}

#[test]
fn chat_response_becomes_anthropic_message() {
    let body = json!({
        "id": "chatcmpl-1",
        "model": "deepseek-chat",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": "hello",
                "tool_calls": [{ "id": "c1", "type": "function", "function": { "name": "Read", "arguments": "{\"path\":\"a\"}" } }]
            },
            "finish_reason": "tool_calls"
        }],
        "usage": { "prompt_tokens": 10, "completion_tokens": 5 }
    });

    let out = convert_response(Direction::AnthropicFromChat, &body);
    assert_eq!(out["type"], "message");
    assert_eq!(out["content"][0]["type"], "text");
    assert_eq!(out["content"][0]["text"], "hello");
    assert_eq!(out["content"][1]["type"], "tool_use");
    assert_eq!(out["content"][1]["name"], "Read");
    assert_eq!(out["content"][1]["input"]["path"], "a");
    assert_eq!(out["stop_reason"], "tool_use");
    assert_eq!(out["usage"]["input_tokens"], 10);
    assert_eq!(out["usage"]["output_tokens"], 5);
}

#[test]
fn chat_stream_becomes_anthropic_events_in_order() {
    let mut converter = StreamConverter::new(Direction::AnthropicFromChat);
    let mut decoder = SseDecoder::new();
    let mut out = Vec::new();

    out.extend(collect(
        &mut converter,
        &mut decoder,
        "data: {\"id\":\"c1\",\"model\":\"deepseek-chat\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"He\"}}]}\n\n",
    ));
    out.extend(collect(
        &mut converter,
        &mut decoder,
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"llo\"}}]}\n\n",
    ));
    out.extend(collect(
        &mut converter,
        &mut decoder,
        "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"completion_tokens\":7}}\n\n",
    ));
    out.extend(drain(&mut converter, &mut decoder));

    let joined = out.join("");
    let order = [
        "message_start",
        "content_block_start",
        "text_delta",
        "content_block_stop",
        "message_delta",
        "message_stop",
    ];
    let mut cursor = 0;
    for needle in order {
        let found = joined[cursor..]
            .find(needle)
            .unwrap_or_else(|| panic!("事件顺序缺了 {needle}：{joined}"));
        cursor += found;
    }
    assert!(joined.contains("\"text\":\"He\""), "增量文本要原样透传");
    assert!(joined.contains("\"text\":\"llo\""));
    assert!(joined.contains("\"stop_reason\":\"end_turn\""));
}

#[test]
fn chat_stream_tool_calls_become_input_json_deltas() {
    let mut converter = StreamConverter::new(Direction::AnthropicFromChat);
    let mut decoder = SseDecoder::new();
    let mut out = Vec::new();

    out.extend(collect(
        &mut converter,
        &mut decoder,
        "data: {\"id\":\"c1\",\"model\":\"m\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"look\"}}]}\n\n",
    ));
    out.extend(collect(
        &mut converter,
        &mut decoder,
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"type\":\"function\",\"function\":{\"name\":\"Read\",\"arguments\":\"{\\\"pa\"}}]}}]}\n\n",
    ));
    out.extend(collect(
        &mut converter,
        &mut decoder,
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"th\\\":\\\"a\\\"}\"}}]}}]}\n\n",
    ));
    out.extend(collect(
        &mut converter,
        &mut decoder,
        "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n",
    ));
    out.extend(drain(&mut converter, &mut decoder));

    let joined = out.join("");
    assert!(joined.contains("\"type\":\"tool_use\""));
    assert!(joined.contains("\"name\":\"Read\""));
    assert_eq!(
        joined.matches("input_json_delta").count(),
        2,
        "两个 arguments 片段要分成两条 delta"
    );
    assert!(joined.contains("\"partial_json\":\"{\\\"pa\""));
    assert!(joined.contains("\"stop_reason\":\"tool_use\""));
    // 文本块必须在工具块之前收尾
    let text_stop = joined.find("content_block_stop").unwrap();
    let tool_start = joined.find("\"type\":\"tool_use\"").unwrap();
    assert!(text_stop < tool_start, "文本块先收尾再开工具块");
}

#[test]
fn chat_request_becomes_anthropic() {
    let body = json!({
        "model": "gpt-6.1",
        "messages": [
            { "role": "system", "content": "sys" },
            { "role": "user", "content": "hi" },
            { "role": "assistant", "content": "", "tool_calls": [
                { "id": "c1", "type": "function", "function": { "name": "Read", "arguments": "{\"p\":1}" } }
            ]},
            { "role": "tool", "tool_call_id": "c1", "content": "done" }
        ]
    });

    let out = convert_request(Direction::ChatFromAnthropic, &body, "claude-sonnet-4-5");
    assert_eq!(out["model"], "claude-sonnet-4-5");
    assert_eq!(out["system"], "sys");
    // 空 assistant 文本只剩 tool_use 块
    assert_eq!(out["messages"][1]["role"], "assistant");
    assert_eq!(out["messages"][1]["content"][0]["type"], "tool_use");
    assert_eq!(out["messages"][1]["content"][0]["input"]["p"], 1);
    // tool 结果并进 user 消息
    assert_eq!(out["messages"][2]["role"], "user");
    assert_eq!(out["messages"][2]["content"][0]["type"], "tool_result");
    assert_eq!(out["messages"][2]["content"][0]["tool_use_id"], "c1");
    assert_eq!(out["max_tokens"], 4096, "Anthropic 侧 max_tokens 必填");
}

#[test]
fn anthropic_response_becomes_chat_completion() {
    let body = json!({
        "id": "msg_1",
        "type": "message",
        "role": "assistant",
        "model": "claude-sonnet-4-5",
        "content": [
            { "type": "text", "text": "hello" },
            { "type": "tool_use", "id": "t1", "name": "Read", "input": { "path": "a" } }
        ],
        "stop_reason": "tool_use",
        "usage": { "input_tokens": 10, "output_tokens": 5 }
    });

    let out = convert_response(Direction::ChatFromAnthropic, &body);
    assert_eq!(out["object"], "chat.completion");
    assert_eq!(out["choices"][0]["message"]["content"], "hello");
    assert_eq!(out["choices"][0]["message"]["tool_calls"][0]["id"], "t1");
    assert_eq!(
        out["choices"][0]["message"]["tool_calls"][0]["function"]["arguments"],
        "{\"path\":\"a\"}"
    );
    assert_eq!(out["choices"][0]["finish_reason"], "tool_calls");
    assert_eq!(out["usage"]["total_tokens"], 15);
}

#[test]
fn anthropic_stream_becomes_chat_chunks() {
    let mut converter = StreamConverter::new(Direction::ChatFromAnthropic);
    let mut decoder = SseDecoder::new();
    let mut out = Vec::new();

    for raw in [
        "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\",\"model\":\"claude-sonnet-4-5\",\"usage\":{\"input_tokens\":9}}}\n\n",
        "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
        "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hi\"}}\n\n",
        "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
        "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":4}}\n\n",
        "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
    ] {
        out.extend(collect(&mut converter, &mut decoder, raw));
    }
    out.extend(drain(&mut converter, &mut decoder));

    let joined = out.join("");
    assert!(joined.contains("\"role\":\"assistant\""));
    assert!(joined.contains("\"content\":\"Hi\""));
    assert!(joined.contains("\"finish_reason\":\"stop\""));
    assert!(joined.contains("[DONE]"), "收尾必须有 [DONE]");
}
