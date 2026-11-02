//! 把上游字节流桥接成「转换后的字节流」。
//!
//! 为什么手写 `Stream`：一个上游块可能解出多条事件、每条事件又可能产出多个下游块，
//! `map` 的 1:1 关系不够用；`flat_map` 的闭包是同步的，拿不到可变的解码器状态。
//!
//! 收尾很重要：上游正常结束、上游报错，两种情况都必须把 `message_stop` / `[DONE]`
//! 补出去，否则 agent 会一直等到超时。

use agenthub_core::l10n::{Msg, t};
use std::collections::VecDeque;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Bytes;
use futures_util::Stream;

use super::{Direction, SseDecoder, StreamConverter};

/// 上游流里的一项。用 trait 而不是多一个泛型参数：
/// 后者会让 `bridge` 的错误类型 `E` 在签名里「只出现一次」，clippy 会拒绝。
pub trait UpstreamItem {
    fn into_result(self) -> Result<Bytes, io::Error>;
}

impl<E: std::fmt::Display> UpstreamItem for Result<Bytes, E> {
    fn into_result(self) -> Result<Bytes, io::Error> {
        self.map_err(|err| io::Error::other(t!(Msg::GatewayStreamBroken, error = err)))
    }
}

struct Inner<S> {
    upstream: S,
    decoder: SseDecoder,
    converter: StreamConverter,
    pending: VecDeque<Result<Bytes, io::Error>>,
    done: bool,
}

impl<S> Stream for Inner<S>
where
    S: Stream + Unpin,
    S::Item: UpstreamItem,
{
    type Item = Result<Bytes, io::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        loop {
            if let Some(item) = this.pending.pop_front() {
                return Poll::Ready(Some(item));
            }
            if this.done {
                return Poll::Ready(None);
            }
            match Pin::new(&mut this.upstream).poll_next(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(None) => {
                    this.done = true;
                    this.flush_tail();
                }
                Poll::Ready(Some(item)) => {
                    let item = item.into_result();
                    // 上游断了：先补收尾（客户端拿到一部分总比干等好），再报错
                    let Ok(chunk) = item else {
                        this.done = true;
                        this.flush_tail();
                        this.pending.push_back(item);
                        continue;
                    };
                    for event in this.decoder.push(&chunk) {
                        this.pending
                            .extend(this.converter.push(&event).into_iter().map(Ok));
                    }
                }
            }
        }
    }
}

impl<S> Inner<S> {
    fn flush_tail(&mut self) {
        for event in self.decoder.finish() {
            self.pending
                .extend(self.converter.push(&event).into_iter().map(Ok));
        }
        self.pending
            .extend(self.converter.finish().into_iter().map(Ok));
    }
}

/// 包一层转换。结果可直接塞进 `axum::body::Body::from_stream`。
pub fn bridge<S>(upstream: S, direction: Direction) -> impl Stream<Item = Result<Bytes, io::Error>>
where
    S: Stream + Unpin,
    S::Item: UpstreamItem,
{
    Inner {
        upstream,
        decoder: SseDecoder::new(),
        converter: StreamConverter::new(direction),
        pending: VecDeque::new(),
        done: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::StreamExt;

    fn chunks(raw: Vec<&'static str>) -> impl Stream<Item = Result<Bytes, String>> + Unpin {
        futures_util::stream::iter(
            raw.into_iter()
                .map(|text| Ok(Bytes::from_static(text.as_bytes())))
                .collect::<Vec<_>>(),
        )
    }

    async fn drain<S>(stream: S) -> (String, bool)
    where
        S: Stream<Item = Result<Bytes, io::Error>> + Unpin,
    {
        let mut stream = stream;
        let mut text = String::new();
        let mut saw_error = false;
        while let Some(item) = stream.next().await {
            match item {
                Ok(bytes) => text.push_str(&String::from_utf8_lossy(&bytes)),
                Err(_) => saw_error = true,
            }
        }
        (text, saw_error)
    }

    #[tokio::test]
    async fn converts_incrementally_and_flushes_at_end() {
        let upstream = chunks(vec![
            "data: {\"id\":\"c1\",\"model\":\"m\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hi\"}}]}\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
            "data: [DONE]\n\n",
        ]);
        let (out, _) = drain(bridge(upstream, Direction::AnthropicFromChat)).await;
        assert!(out.contains("message_start"));
        assert!(out.contains("\"text\":\"Hi\""));
        assert!(out.contains("message_stop"), "收尾事件不能丢");
    }

    #[tokio::test]
    async fn split_event_across_chunks_is_still_converted() {
        let upstream = chunks(vec![
            "data: {\"id\":\"c1\",\"model\":\"m\",\"choices\":[{\"index\":0,\"delta\":{\"cont",
            "ent\":\"ok\"}}]}\n\ndata: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
        ]);
        let (out, _) = drain(bridge(upstream, Direction::AnthropicFromChat)).await;
        assert!(out.contains("\"text\":\"ok\""));
        assert!(out.contains("message_stop"));
    }

    #[tokio::test]
    async fn upstream_error_still_finishes_the_stream() {
        let upstream = futures_util::stream::iter(vec![
            Ok::<Bytes, String>(Bytes::from_static(
                b"data: {\"id\":\"c1\",\"model\":\"m\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"a\"}}]}\n\n",
            )),
            Err("连接被重置".to_string()),
        ]);
        let (text, saw_error) =
            drain(bridge(Box::pin(upstream), Direction::AnthropicFromChat)).await;
        assert!(text.contains("message_stop"), "断流前也要把收尾补出来");
        assert!(saw_error);
    }
}
