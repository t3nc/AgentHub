//! 跨协议转换：Anthropic Messages ⇄ OpenAI Chat Completions。
//!
//! 只有这两个方向。涉及 Responses 的组合在保存配置时就被 `protocol_compatible`
//! 拦掉了，不会走到这里。
//!
//! 分工：
//! - `sse`：增量解码 / 编码，屏蔽「一个块里多条事件」与「一条事件跨两个块」；
//! - `chat_to_anthropic`：上游 Chat、下游 Anthropic（Claude Code 接国产厂商）；
//! - `anthropic_to_chat`：上游 Anthropic、下游 Chat（反向）。

pub mod anthropic_to_chat;
pub mod chat_to_anthropic;
pub mod sse;
pub mod stream_bridge;

use agenthub_core::model::gateway::GatewayProtocol;
use bytes::Bytes;

pub use sse::{SseDecoder, SseEvent};

/// 一次转发要做的转换方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// 客户端说 Anthropic，上游只会 Chat
    AnthropicFromChat,
    /// 客户端说 Chat，上游只会 Anthropic
    ChatFromAnthropic,
}

/// 同协议返回 None（走字节级透传）。
pub fn direction(downstream: GatewayProtocol, upstream: GatewayProtocol) -> Option<Direction> {
    use GatewayProtocol::{Anthropic, ChatCompletions};
    match (downstream, upstream) {
        (Anthropic, ChatCompletions) => Some(Direction::AnthropicFromChat),
        (ChatCompletions, Anthropic) => Some(Direction::ChatFromAnthropic),
        _ => None,
    }
}

/// 转换方向上，上游该打哪个路径。
pub fn upstream_path(direction: Direction) -> &'static str {
    match direction {
        Direction::AnthropicFromChat => "/v1/chat/completions",
        Direction::ChatFromAnthropic => "/v1/messages",
    }
}

/// 请求体转换。`model` 是上游要用的模型名。
pub fn convert_request(
    direction: Direction,
    body: &serde_json::Value,
    model: &str,
) -> serde_json::Value {
    match direction {
        Direction::AnthropicFromChat => chat_to_anthropic::request(body, model),
        Direction::ChatFromAnthropic => anthropic_to_chat::request(body, model),
    }
}

/// 非流式响应转换。
pub fn convert_response(direction: Direction, body: &serde_json::Value) -> serde_json::Value {
    match direction {
        Direction::AnthropicFromChat => chat_to_anthropic::response(body),
        Direction::ChatFromAnthropic => anthropic_to_chat::response(body),
    }
}

/// 流式转换的两种状态机。
pub enum StreamConverter {
    ToAnthropic(chat_to_anthropic::Stream),
    ToChat(anthropic_to_chat::Stream),
}

impl StreamConverter {
    pub fn new(direction: Direction) -> Self {
        match direction {
            Direction::AnthropicFromChat => Self::ToAnthropic(chat_to_anthropic::Stream::new()),
            Direction::ChatFromAnthropic => Self::ToChat(anthropic_to_chat::Stream::new()),
        }
    }

    /// 喂一个上游事件，吐出 0..n 个要写给客户端的块。
    pub fn push(&mut self, event: &SseEvent) -> Vec<Bytes> {
        match self {
            Self::ToAnthropic(state) => state.push(event),
            Self::ToChat(state) => state.push(event),
        }
    }

    /// 上游结束：必须调用，否则客户端等不到收尾事件。
    pub fn finish(&mut self) -> Vec<Bytes> {
        match self {
            Self::ToAnthropic(state) => state.finish(),
            Self::ToChat(state) => state.finish(),
        }
    }
}

#[cfg(test)]
mod tests;
