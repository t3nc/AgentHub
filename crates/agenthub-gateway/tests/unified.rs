//! 统一入口 `/v1`：令牌即 agent 名，网关靠它选中「供应商池」。
//!
//! 同时确认老的 `/gw/<route>` 路径仍然可用（上一轮已经写进用户配置里了）。

use std::collections::BTreeMap;
use std::net::SocketAddr;

use agenthub_core::config::AppConfig;
use agenthub_core::ext::provider::Provider;
use agenthub_core::model::gateway::{
    GatewayProtocol, GatewayRoute, RetryPolicy, RouteMember, RouteMode,
};
use agenthub_core::model::mcp::SecretRef;
use agenthub_gateway::{GatewayHandle, Snapshot};
use axum::Router;
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use axum::routing::any;
use serde_json::json;

/// 上游只回一句话：把收到的密钥原样带回来，方便断言「入站令牌被换成了上游密钥」。
async fn spawn_upstream() -> SocketAddr {
    let handler = |headers: HeaderMap| async move {
        let key = headers
            .get("x-api-key")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("<none>");
        axum::Json(json!({ "ok": true, "upstream_key": key })).into_response()
    };
    let app = Router::new().fallback(any(handler));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    addr
}

fn provider(id: &str, base_url: &str) -> Provider {
    Provider {
        id: id.to_string(),
        name: id.to_uppercase(),
        base_url: base_url.to_string(),
        api_key: SecretRef::Literal {
            value: "sk-upstream".into(),
            acknowledged: true,
        },
        models: vec!["m1".into()],
        note: None,
        protocol: None,
        model_limits: BTreeMap::new(),
        catalog_id: None,
    }
}

/// 一条 id 等于 agent slug 的池路由，正是新布局写出来的形态。
fn pool_route(id: &str) -> GatewayRoute {
    GatewayRoute {
        id: id.into(),
        name: format!("{id} 供应商池"),
        protocol: GatewayProtocol::Anthropic,
        mode: RouteMode::Route,
        members: vec![RouteMember {
            provider_id: "relay".into(),
            weight: 1,
            priority: 1,
            models: Vec::new(),
            model_map: BTreeMap::new(),
            enabled: true,
        }],
        rules: Vec::new(),
        retry: RetryPolicy::default(),
        enabled: true,
    }
}

async fn start(routes: Vec<GatewayRoute>, upstream: SocketAddr) -> GatewayHandle {
    let config = AppConfig {
        providers: vec![provider("relay", &format!("http://{upstream}"))],
        gateway_routes: routes,
        ..AppConfig::default()
    };
    let (tx, _rx) = agenthub_gateway::mpsc::unbounded_channel();
    GatewayHandle::start("127.0.0.1", 0, Snapshot::compile(&config), tx)
        .await
        .expect("gateway should start")
}

fn url(handle: &GatewayHandle, path: &str) -> String {
    format!("http://{}{}", handle.addr(), path)
}

#[tokio::test]
async fn bearer_token_selects_the_pool() {
    let upstream = spawn_upstream().await;
    let handle = start(vec![pool_route("claude"), pool_route("codex")], upstream).await;
    let client = reqwest::Client::new();

    let response = client
        .post(url(&handle, "/v1/messages"))
        .bearer_auth("claude")
        .json(&json!({ "model": "m1" }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(
        body["upstream_key"], "sk-upstream",
        "令牌只用来选池，转发时仍要换成上游密钥"
    );
}

#[tokio::test]
async fn unknown_token_lists_available_pools() {
    let upstream = spawn_upstream().await;
    let handle = start(vec![pool_route("claude"), pool_route("codex")], upstream).await;
    let client = reqwest::Client::new();

    let response = client
        .post(url(&handle, "/v1/messages"))
        .bearer_auth("nope")
        .json(&json!({ "model": "m1" }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 401);
    let body: serde_json::Value = response.json().await.unwrap();
    let tokens = body["tokens"].as_array().expect("应返回可用令牌列表");
    assert!(tokens.iter().any(|item| item == "claude"));
    assert!(tokens.iter().any(|item| item == "codex"));
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap_or("")
            .contains("claude"),
        "错误文案要直接告诉用户该填什么：{body}"
    );
}

#[tokio::test]
async fn missing_token_is_rejected() {
    let upstream = spawn_upstream().await;
    let handle = start(vec![pool_route("claude")], upstream).await;

    let response = reqwest::Client::new()
        .post(url(&handle, "/v1/messages"))
        .json(&json!({ "model": "m1" }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn legacy_gw_path_still_works() {
    let upstream = spawn_upstream().await;
    let handle = start(vec![pool_route("claude")], upstream).await;

    let response = reqwest::Client::new()
        .post(url(&handle, "/gw/claude/v1/messages"))
        .json(&json!({ "model": "m1" }))
        .send()
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        200,
        "上一轮已经把 /gw/<route> 写进用户配置，不能一升级就断"
    );
}
