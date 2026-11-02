//! 端到端：起一个本地 fake upstream，让网关真的转一次。
//!
//! 覆盖四个最容易回归的点：认证头替换、SSE 逐块顺序、503 故障转移、本地令牌。

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use agenthub_core::config::AppConfig;
use agenthub_core::ext::provider::Provider;
use agenthub_core::model::gateway::{
    GatewayProtocol, GatewayRoute, RetryPolicy, RouteMember, RouteMode,
};
use agenthub_core::model::mcp::SecretRef;
use agenthub_gateway::{GatewayHandle, Snapshot};
use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::any;
use serde_json::json;
use tokio::sync::mpsc::{self, UnboundedReceiver};

struct Mock {
    hits: AtomicU32,
    /// 前 N 次返回 503，用来验证故障转移
    fail_first: u32,
    /// 返回 SSE 而不是普通 JSON
    sse: bool,
}

async fn mock_handler(State(mock): State<Arc<Mock>>, headers: HeaderMap, body: Bytes) -> Response {
    let hit = mock.hits.fetch_add(1, Ordering::SeqCst);
    if hit < mock.fail_first {
        return (StatusCode::SERVICE_UNAVAILABLE, "upstream busy").into_response();
    }

    if mock.sse {
        let chunks = vec![
            Ok::<Bytes, std::io::Error>(Bytes::from("data: chunk-1\n\n")),
            Ok(Bytes::from("data: chunk-2\n\n")),
        ];
        let stream = futures_util::stream::iter(chunks);
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/event-stream")
            .body(Body::from_stream(stream))
            .expect("build sse response");
    }

    let key = headers
        .get("x-api-key")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("<none>");
    axum::Json(json!({
        "ok": true,
        "upstream_key": key,
        "body": String::from_utf8_lossy(&body),
    }))
    .into_response()
}

async fn spawn_upstream(fail_first: u32, sse: bool) -> SocketAddr {
    let mock = Arc::new(Mock {
        hits: AtomicU32::new(0),
        fail_first,
        sse,
    });
    let app = Router::new().fallback(any(mock_handler)).with_state(mock);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    addr
}

/// 一个只会 Chat Completions 的上游：非流式返回 Chat JSON，流式返回 Chat SSE。
async fn spawn_chat_upstream(sse: bool) -> SocketAddr {
    let handler = move |_body: Bytes| async move {
        if sse {
            let chunks = vec![
                Ok::<Bytes, std::io::Error>(Bytes::from(
                    "data: {\"id\":\"c1\",\"model\":\"m1\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"he\"}}]}\n\n",
                )),
                Ok(Bytes::from(
                    "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
                )),
                Ok(Bytes::from("data: [DONE]\n\n")),
            ];
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/event-stream")
                .body(Body::from_stream(futures_util::stream::iter(chunks)))
                .expect("build sse response")
        } else {
            axum::Json(json!({
                "id": "c1",
                "model": "m1",
                "choices": [{
                    "index": 0,
                    "message": { "role": "assistant", "content": "hi from chat" },
                    "finish_reason": "stop"
                }],
                "usage": { "prompt_tokens": 3, "completion_tokens": 2 }
            }))
            .into_response()
        }
    };
    let app = Router::new().fallback(any(handler));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    addr
}

fn chat_provider(id: &str, base_url: &str, key: &str) -> Provider {
    let mut provider = provider(id, base_url, key);
    provider.protocol = Some(GatewayProtocol::ChatCompletions);
    provider
}

fn provider(id: &str, base_url: &str, key: &str) -> Provider {
    Provider {
        id: id.to_string(),
        name: id.to_uppercase(),
        base_url: base_url.to_string(),
        api_key: SecretRef::Literal {
            value: key.to_string(),
            acknowledged: true,
        },
        models: vec!["m1".into(), "m2".into()],
        note: None,
        protocol: None,
        model_limits: std::collections::BTreeMap::new(),
        catalog_id: None,
    }
}

fn member(id: &str, weight: u32, priority: u32) -> RouteMember {
    RouteMember {
        provider_id: id.to_string(),
        weight,
        priority,
        models: Vec::new(),
        model_map: BTreeMap::new(),
        enabled: true,
    }
}

fn route(mode: RouteMode, members: Vec<RouteMember>) -> GatewayRoute {
    GatewayRoute {
        id: "main".into(),
        name: "主路由".into(),
        protocol: GatewayProtocol::Anthropic,
        mode,
        members,
        rules: Vec::new(),
        retry: RetryPolicy {
            max_attempts: 3,
            ..RetryPolicy::default()
        },
        enabled: true,
    }
}

fn config(providers: Vec<Provider>, route: GatewayRoute, token: Option<&str>) -> AppConfig {
    let mut config = AppConfig {
        providers,
        gateway_routes: vec![route],
        ..AppConfig::default()
    };
    config.gateway.auth_token = token.map(str::to_string);
    config
}

async fn start(
    config: AppConfig,
) -> (
    GatewayHandle,
    UnboundedReceiver<agenthub_gateway::RequestLog>,
) {
    let (tx, rx) = mpsc::unbounded_channel();
    let handle = GatewayHandle::start("127.0.0.1", 0, Snapshot::compile(&config), tx)
        .await
        .expect("gateway should start");
    (handle, rx)
}

fn gw(handle: &GatewayHandle, path: &str) -> String {
    format!("http://{}{}", handle.addr(), path)
}

#[tokio::test]
async fn forwards_request_and_injects_upstream_key() {
    let upstream = spawn_upstream(0, false).await;
    let config = config(
        vec![provider("a", &format!("http://{upstream}"), "sk-upstream")],
        route(RouteMode::Single, vec![member("a", 1, 1)]),
        None,
    );
    let (handle, _logs) = start(config).await;
    let client = reqwest::Client::new();

    let response = client
        .post(gw(&handle, "/gw/main/v1/messages"))
        .header("x-api-key", "local-token")
        .header("anthropic-version", "2023-06-01")
        .json(&json!({ "model": "m1", "stream": false }))
        .send()
        .await
        .expect("request should reach gateway");

    let status = response.status();
    let raw = response.text().await.unwrap();
    assert_eq!(status, 200, "网关返回了非 200：{raw}");
    let body: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(
        body["upstream_key"], "sk-upstream",
        "入站的本地令牌必须被替换成上游密钥"
    );
    assert!(body["body"].as_str().unwrap().contains("\"m1\""));
}

#[tokio::test]
async fn retries_on_503_before_first_byte() {
    let flaky = spawn_upstream(2, false).await;
    let config = config(
        vec![provider("a", &format!("http://{flaky}"), "sk-a")],
        route(RouteMode::Single, vec![member("a", 1, 1)]),
        None,
    );
    let (handle, mut logs) = start(config).await;

    let response = reqwest::Client::new()
        .post(gw(&handle, "/gw/main/v1/messages"))
        .json(&json!({ "model": "m1" }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200, "前两次 503 应被重试吃掉");
    let log = logs.recv().await.expect("一次请求应产生一行日志");
    assert_eq!(log.attempts, 3);
    assert_eq!(log.status, 200);
    assert_eq!(log.provider_id.as_deref(), Some("a"));
}

#[tokio::test]
async fn streams_sse_chunks_in_order() {
    let upstream = spawn_upstream(0, true).await;
    let config = config(
        vec![provider("a", &format!("http://{upstream}"), "sk-a")],
        route(RouteMode::Single, vec![member("a", 1, 1)]),
        None,
    );
    let (handle, _logs) = start(config).await;

    let response = reqwest::Client::new()
        .post(gw(&handle, "/gw/main/v1/messages"))
        .json(&json!({ "model": "m1", "stream": true }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok()),
        Some("text/event-stream")
    );
    let text = response.text().await.unwrap();
    assert_eq!(text, "data: chunk-1\n\ndata: chunk-2\n\n");
}

#[tokio::test]
async fn requires_local_token_when_configured() {
    let upstream = spawn_upstream(0, false).await;
    let config = config(
        vec![provider("a", &format!("http://{upstream}"), "sk-a")],
        route(RouteMode::Single, vec![member("a", 1, 1)]),
        Some("local-token"),
    );
    let (handle, _logs) = start(config).await;
    let client = reqwest::Client::new();

    let denied = client
        .post(gw(&handle, "/gw/main/v1/messages"))
        .json(&json!({ "model": "m1" }))
        .send()
        .await
        .unwrap();
    assert_eq!(denied.status(), 401);

    let allowed = client
        .post(gw(&handle, "/gw/main/v1/messages"))
        .bearer_auth("local-token")
        .json(&json!({ "model": "m1" }))
        .send()
        .await
        .unwrap();
    assert_eq!(allowed.status(), 200);
}

#[tokio::test]
async fn merges_model_pool_locally() {
    let upstream = spawn_upstream(0, false).await;
    let config = config(
        vec![provider("a", &format!("http://{upstream}"), "sk-a")],
        route(RouteMode::Single, vec![member("a", 1, 1)]),
        None,
    );
    let (handle, _logs) = start(config).await;

    let response = reqwest::Client::new()
        .get(gw(&handle, "/gw/main/v1/models"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    let ids: Vec<&str> = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["m1", "m2"]);
}

/// 下游 Anthropic、上游 Chat：非流式响应要转成 Anthropic message。
#[tokio::test]
async fn converts_chat_upstream_to_anthropic_response() {
    let upstream = spawn_chat_upstream(false).await;
    let config = config(
        vec![chat_provider(
            "a",
            &format!("http://{upstream}"),
            "sk-upstream",
        )],
        route(RouteMode::Single, vec![member("a", 1, 1)]),
        None,
    );
    let (handle, _logs) = start(config).await;

    let response = reqwest::Client::new()
        .post(gw(&handle, "/gw/main/v1/messages"))
        .json(&json!({ "model": "m1", "max_tokens": 64 }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["type"], "message", "客户端必须收到 Anthropic 形态");
    assert_eq!(body["content"][0]["type"], "text");
    assert_eq!(body["content"][0]["text"], "hi from chat");
    assert_eq!(body["stop_reason"], "end_turn");
    assert_eq!(body["usage"]["input_tokens"], 3);
    assert_eq!(body["usage"]["output_tokens"], 2);
}

/// 下游 Anthropic、上游 Chat 的流式：逐块转成 Anthropic 事件序列。
#[tokio::test]
async fn converts_chat_stream_to_anthropic_events() {
    let upstream = spawn_chat_upstream(true).await;
    let config = config(
        vec![chat_provider(
            "a",
            &format!("http://{upstream}"),
            "sk-upstream",
        )],
        route(RouteMode::Single, vec![member("a", 1, 1)]),
        None,
    );
    let (handle, _logs) = start(config).await;

    let response = reqwest::Client::new()
        .post(gw(&handle, "/gw/main/v1/messages"))
        .json(&json!({ "model": "m1", "stream": true }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok()),
        Some("text/event-stream")
    );
    let text = response.text().await.unwrap();
    assert!(
        text.contains("event: message_start"),
        "缺少 message_start：{text}"
    );
    assert!(text.contains("\"text\":\"he\""), "文本增量没转出来：{text}");
    assert!(
        text.contains("event: message_stop"),
        "缺少 message_stop：{text}"
    );
    assert!(
        !text.contains("[DONE]"),
        "Anthropic 客户端不认识 [DONE]，不该透传：{text}"
    );
}

#[tokio::test]
async fn unknown_route_returns_404() {
    let config = config(Vec::new(), route(RouteMode::Single, Vec::new()), None);
    let (handle, _logs) = start(config).await;
    let response = reqwest::get(gw(&handle, "/gw/nope/v1/models"))
        .await
        .unwrap();
    assert_eq!(response.status(), 404);
}
