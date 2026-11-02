//! SSE 增量解码与编码。
//!
//! 必须处理两种切法：一个网络块里含**多条**事件、一条事件被切成**两个**块。
//! 所以解码器只按行推进，未闭合的半截留在缓冲里等下一块。

use bytes::Bytes;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEvent {
    /// `event: xxx`，没有就是 None
    pub event: Option<String>,
    /// 多行 `data:` 用 `\n` 连接
    pub data: String,
}

impl SseEvent {
    pub fn is_done(&self) -> bool {
        self.data.trim() == "[DONE]"
    }
}

#[derive(Default)]
pub struct SseDecoder {
    buf: String,
    event: Option<String>,
    data: Vec<String>,
}

impl SseDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    /// 喂一块，返回其中已经完整的事件。
    pub fn push(&mut self, chunk: &[u8]) -> Vec<SseEvent> {
        // SSE 是 UTF-8 文本；用 lossy 是为了不因为半截多字节字符卡住整条流
        self.buf.push_str(&String::from_utf8_lossy(chunk));

        let mut out = Vec::new();
        while let Some(index) = self.buf.find('\n') {
            let line = self.buf[..index].trim_end_matches('\r').to_string();
            self.buf.drain(..=index);

            if line.is_empty() {
                if let Some(event) = self.flush() {
                    out.push(event);
                }
                continue;
            }
            // 注释行（`: ping`）与未知字段直接忽略
            if line.starts_with(':') {
                continue;
            }
            let (field, value) = match line.split_once(':') {
                Some((field, value)) => (field, value.strip_prefix(' ').unwrap_or(value)),
                None => (line.as_str(), ""),
            };
            match field {
                "event" => self.event = Some(value.to_string()),
                // 多行 data 按 SSE 规范用换行拼起来
                "data" => self.data.push(value.to_string()),
                _ => {}
            }
        }
        out
    }

    /// 流结束：把最后一条没以空行收尾的事件冲出来。
    pub fn finish(&mut self) -> Vec<SseEvent> {
        let mut out = Vec::new();
        if !self.buf.is_empty() {
            let rest = std::mem::take(&mut self.buf);
            for line in rest.lines() {
                let line = line.trim_end_matches('\r');
                if let Some(value) = line.strip_prefix("data:") {
                    self.data
                        .push(value.strip_prefix(' ').unwrap_or(value).to_string());
                }
            }
        }
        if let Some(event) = self.flush() {
            out.push(event);
        }
        out
    }

    fn flush(&mut self) -> Option<SseEvent> {
        if self.data.is_empty() && self.event.is_none() {
            return None;
        }
        let data = std::mem::take(&mut self.data).join("\n");
        let event = self.event.take();
        Some(SseEvent { event, data })
    }
}

/// 编码一条 SSE 事件。带 `event:` 的用于 Anthropic，仅 `data:` 的用于 OpenAI。
pub fn encode(event: Option<&str>, payload: &serde_json::Value) -> Bytes {
    let data = serde_json::to_string(payload).unwrap_or_else(|_| "{}".into());
    encode_raw(event, &data)
}

pub fn encode_raw(event: Option<&str>, data: &str) -> Bytes {
    match event {
        Some(name) => Bytes::from(format!("event: {name}\ndata: {data}\n\n")),
        None => Bytes::from(format!("data: {data}\n\n")),
    }
}

pub const DONE: &str = "data: [DONE]\n\n";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_multiple_events_in_one_chunk() {
        let mut decoder = SseDecoder::new();
        let events = decoder.push(b"event: a\ndata: 1\n\nevent: b\ndata: 2\n\n");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event.as_deref(), Some("a"));
        assert_eq!(events[0].data, "1");
        assert_eq!(events[1].event.as_deref(), Some("b"));
    }

    #[test]
    fn waits_for_the_rest_of_a_split_event() {
        let mut decoder = SseDecoder::new();
        assert!(decoder.push(b"data: {\"a\":").is_empty(), "半截不能吐出去");
        let events = decoder.push(b"1}\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "{\"a\":1}");
    }

    #[test]
    fn joins_multi_line_data() {
        let mut decoder = SseDecoder::new();
        let events = decoder.push(b"data: line1\ndata: line2\n\n");
        assert_eq!(events[0].data, "line1\nline2");
    }

    #[test]
    fn ignores_comments_and_flushes_tail() {
        let mut decoder = SseDecoder::new();
        assert!(decoder.push(b": ping\n\n").is_empty(), "注释不产生事件");
        let tail = decoder.push(b"data: [DONE]\n");
        assert!(tail.is_empty(), "没有空行结尾时先攒着");
        let tail = decoder.finish();
        assert_eq!(tail.len(), 1);
        assert!(tail[0].is_done());
    }

    #[test]
    fn encodes_both_shapes() {
        let with_event = encode(
            Some("message_stop"),
            &serde_json::json!({"type": "message_stop"}),
        );
        assert!(String::from_utf8_lossy(&with_event).starts_with("event: message_stop\ndata: "));
        let plain = encode(None, &serde_json::json!({"x": 1}));
        assert!(String::from_utf8_lossy(&plain).starts_with("data: {"));
    }
}
