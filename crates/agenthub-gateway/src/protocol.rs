//! 规范化中间请求 + 协议适配器。
//!
//! 本期只有「恒等透传」：不做 Messages⇄Chat 的字段映射。但请求先被拆成
//! `CanonicalRequest` 再交给 `ProtocolAdapter`，将来加转换只需要新增一个 impl，
//! 转发与重试管线一行不用改。
//!
//! **字节保真**：没有模型别名时请求体原样转发（不经过 JSON 反序列化再序列化），
//! 否则上游收到的字节会变，签名校验与 prompt 缓存命中都会受影响。

use std::collections::BTreeMap;

use agenthub_core::model::gateway::GatewayProtocol;
use bytes::Bytes;

/// 一个已经被读懂「够用部分」的入站请求。
#[derive(Debug, Clone)]
pub struct CanonicalRequest {
    pub protocol: GatewayProtocol,
    pub method: String,
    /// 上游路径（已剥掉 `/gw/<route>` 前缀），以 `/` 开头
    pub path: String,
    /// 原始 query（不含 `?`）
    pub query: Option<String>,
    pub model: Option<String>,
    pub stream: bool,
    /// 未改写的原始字节
    pub body: Bytes,
}

impl CanonicalRequest {
    pub fn parse(
        protocol: GatewayProtocol,
        method: &str,
        path: &str,
        query: Option<&str>,
        body: Bytes,
    ) -> Self {
        let (model, stream) = sniff(&body);
        Self {
            protocol,
            method: method.to_string(),
            path: path.to_string(),
            query: query.filter(|q| !q.is_empty()).map(str::to_string),
            model,
            stream,
            body,
        }
    }

    /// 是否为模型列表请求（由网关本地合并，不打上游）。
    pub fn is_models(&self) -> bool {
        self.method.eq_ignore_ascii_case("GET")
            && (self.path == "/v1/models" || self.path == "/models")
    }
}

/// 只读 `model` / `stream` 两个字段，不做完整校验。
///
/// 解析失败（非 JSON、非对象）时两者都是 None —— 请求照样转发，只是无法选路，
/// 会落到「按优先级兜底」。
fn sniff(body: &Bytes) -> (Option<String>, bool) {
    if body.is_empty() {
        return (None, false);
    }
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return (None, false);
    };
    let Some(object) = value.as_object() else {
        return (None, false);
    };
    let model = object
        .get("model")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let stream = object
        .get("stream")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    (model, stream)
}

/// 按别名表改写 `model` 字段。没有映射时返回原字节（保证上游收到的内容不变）。
pub fn rewrite_model(body: &Bytes, model: Option<&str>, map: &BTreeMap<String, String>) -> Bytes {
    let Some(model) = model else {
        return body.clone();
    };
    let Some(target) = map.get(model) else {
        return body.clone();
    };
    let Ok(mut value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return body.clone();
    };
    let Some(object) = value.as_object_mut() else {
        return body.clone();
    };
    object.insert(
        "model".to_string(),
        serde_json::Value::String(target.clone()),
    );
    match serde_json::to_vec(&value) {
        Ok(bytes) => Bytes::from(bytes),
        Err(_) => body.clone(),
    }
}

/// 协议适配器：把规范化请求变成「要发给上游的字节 + 需要覆盖的头」。
///
/// 本期两个实现都是恒等，将来加 `OpenAiToAnthropic` 之类的转换只在这里扩展。
pub trait ProtocolAdapter: Send + Sync {
    fn prepare(
        &self,
        req: &CanonicalRequest,
        model_map: &BTreeMap<String, String>,
    ) -> (Bytes, Vec<(String, String)>);
}

/// 同协议透传。
pub struct Passthrough;

impl ProtocolAdapter for Passthrough {
    fn prepare(
        &self,
        req: &CanonicalRequest,
        model_map: &BTreeMap<String, String>,
    ) -> (Bytes, Vec<(String, String)>) {
        let body = rewrite_model(&req.body, req.model.as_deref(), model_map);
        (body, Vec::new())
    }
}

static PASSTHROUGH: Passthrough = Passthrough;

pub fn adapter_for(_protocol: GatewayProtocol) -> &'static dyn ProtocolAdapter {
    &PASSTHROUGH
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map() -> BTreeMap<String, String> {
        let mut map = BTreeMap::new();
        map.insert("my-model".into(), "claude-sonnet-4-5".into());
        map
    }

    #[test]
    fn sniffs_model_and_stream() {
        let body = Bytes::from(r#"{"model":"claude-opus-4","stream":true,"x":1}"#);
        let req = CanonicalRequest::parse(
            GatewayProtocol::Anthropic,
            "POST",
            "/v1/messages",
            None,
            body,
        );
        assert_eq!(req.model.as_deref(), Some("claude-opus-4"));
        assert!(req.stream);
    }

    #[test]
    fn keeps_bytes_when_no_alias() {
        let original = Bytes::from(r#"{"model":"other","stream":false}"#);
        let out = rewrite_model(&original, Some("other"), &map());
        assert_eq!(out, original, "没有别名时必须字节级不变");
    }

    #[test]
    fn rewrites_only_the_model_field() {
        let original = Bytes::from(r#"{"model":"my-model","stream":true}"#);
        let out = rewrite_model(&original, Some("my-model"), &map());
        let value: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(value["model"], "claude-sonnet-4-5");
        assert_eq!(value["stream"], true);
    }

    #[test]
    fn non_json_body_passes_through() {
        let original = Bytes::from("not json at all");
        assert_eq!(rewrite_model(&original, None, &map()), original);
        let req = CanonicalRequest::parse(
            GatewayProtocol::ChatCompletions,
            "POST",
            "/v1/chat/completions",
            None,
            original,
        );
        assert!(req.model.is_none());
    }

    #[test]
    fn models_path_is_detected() {
        let req = CanonicalRequest::parse(
            GatewayProtocol::ChatCompletions,
            "GET",
            "/v1/models",
            None,
            Bytes::new(),
        );
        assert!(req.is_models());
    }
}
