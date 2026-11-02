//! 上游连接：客户端、URL 拼接、请求头与响应头过滤。

use std::time::Duration;

use agenthub_core::ext::gateway::join_upstream_url;
use agenthub_core::model::gateway::GatewayProtocol;
use axum::http::{HeaderMap, HeaderName, HeaderValue};
use reqwest::Client;

/// 连接池复用 + 关闭自动解压。
///
/// 自动解压会让上游的 `content-encoding` 与实际字节不一致（我们转发的是解压后的
/// 内容，却不能告诉客户端它是明文），SSE 场景下更会把分块攒成一整段。
/// 所以干脆全部关掉，压缩与否交给两端自己协商。
pub fn build_client() -> Client {
    match Client::builder()
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .no_zstd()
        .pool_idle_timeout(Duration::from_secs(90))
        .pool_max_idle_per_host(8)
        .tcp_nodelay(true)
        .build()
    {
        Ok(client) => client,
        Err(_) => Client::new(),
    }
}

const HOP_BY_HOP: [&str; 8] = [
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
];

fn is_hop_by_hop(name: &str) -> bool {
    HOP_BY_HOP.contains(&name)
}

/// 构造要发给上游的请求头。
///
/// 入站的认证头一律丢弃 —— 客户端带来的是网关自己的本地令牌（或什么都没有），
/// 真密钥只在转发瞬间注入。
pub fn request_headers(
    src: &HeaderMap,
    protocol: GatewayProtocol,
    api_key: Option<&str>,
) -> HeaderMap {
    let mut out = HeaderMap::new();
    for (name, value) in src.iter() {
        let key = name.as_str();
        if is_hop_by_hop(key) || key == "host" || key == "content-length" {
            continue;
        }
        if key == "authorization" || key == "x-api-key" {
            continue;
        }
        out.append(name.clone(), value.clone());
    }
    if let Some(key) = api_key.filter(|k| !k.is_empty()) {
        match protocol {
            GatewayProtocol::Anthropic => {
                if let Ok(value) = HeaderValue::from_str(key) {
                    out.insert(HeaderName::from_static("x-api-key"), value);
                }
            }
            // Chat Completions 与 Responses 共用 Bearer 鉴权
            GatewayProtocol::ChatCompletions | GatewayProtocol::Responses => {
                if let Ok(value) = HeaderValue::from_str(&format!("Bearer {key}")) {
                    out.insert(HeaderName::from_static("authorization"), value);
                }
            }
        }
    }
    if !out.contains_key("content-type") {
        out.insert(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("application/json"),
        );
    }
    out
}

/// 回写给客户端的响应头。
///
/// 去掉长度与传输编码（body 由 axum 重新分块）；`content-encoding` 保留，
/// 因为客户端拿到的就是上游原始字节。
pub fn response_headers(src: &HeaderMap) -> HeaderMap {
    let mut out = HeaderMap::new();
    for (name, value) in src.iter() {
        let key = name.as_str();
        if is_hop_by_hop(key) || key == "content-length" || key == "connection" {
            continue;
        }
        out.append(name.clone(), value.clone());
    }
    out
}

/// 上游完整 URL：base_url + 路径 + query。
pub fn upstream_url(base_url: &str, path: &str, query: Option<&str>) -> String {
    let base = join_upstream_url(base_url, path);
    match query.filter(|q| !q.is_empty()) {
        Some(q) => {
            let sep = if base.contains('?') { '&' } else { '?' };
            format!("{base}{sep}{q}")
        }
        None => base,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_keeps_query() {
        assert_eq!(
            upstream_url(
                "https://relay.example.com/v1",
                "/v1/messages",
                Some("beta=1")
            ),
            "https://relay.example.com/v1/messages?beta=1"
        );
        assert_eq!(
            upstream_url("https://api.openai.com", "/v1/models", None),
            "https://api.openai.com/v1/models"
        );
    }

    #[test]
    fn request_headers_replace_auth() {
        let mut src = HeaderMap::new();
        src.insert(
            "authorization",
            HeaderValue::from_static("Bearer local-token"),
        );
        src.insert("x-api-key", HeaderValue::from_static("local"));
        src.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
        src.insert("host", HeaderValue::from_static("127.0.0.1:43117"));

        let out = request_headers(&src, GatewayProtocol::Anthropic, Some("sk-upstream"));
        assert_eq!(out.get("x-api-key").unwrap(), "sk-upstream");
        assert!(out.get("authorization").is_none(), "入站认证头必须丢弃");
        assert!(out.get("host").is_none());
        assert_eq!(out.get("anthropic-version").unwrap(), "2023-06-01");
    }

    #[test]
    fn response_headers_drop_length() {
        let mut src = HeaderMap::new();
        src.insert("content-length", HeaderValue::from_static("123"));
        src.insert(
            "content-type",
            HeaderValue::from_static("text/event-stream"),
        );
        let out = response_headers(&src);
        assert!(out.get("content-length").is_none());
        assert_eq!(out.get("content-type").unwrap(), "text/event-stream");
    }
}
