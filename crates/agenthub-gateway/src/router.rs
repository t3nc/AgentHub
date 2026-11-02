//! axum 路由表与鉴权中间件。
//!
//! 三个入口：
//! - `/healthz`：免鉴权，给启动探活用；
//! - `/gw/{route_id}/...`：按路径段指定路由（老形态，保留兼容）；
//! - `/v1/...`：**统一入口**，路由由 `Authorization: Bearer <token>` 决定，
//!   令牌即 agent 的供应商池 id（`claude` / `codex` / …）。
//!
//! 所有业务路径都通过 `OriginalUri` 原样透传，不在这一层做路径重写。

use std::sync::Arc;

use agenthub_core::l10n::{Msg, t};
use axum::Router;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, OriginalUri, Request, State as AxumState};
use axum::http::{HeaderMap, Method, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get};
use serde_json::json;

use crate::State;
use crate::config::DEFAULT_MAX_BODY;
use crate::proxy;

pub fn build(state: Arc<State>) -> Router {
    Router::new()
        .route("/healthz", get(health))
        .route("/gw/{route_id}", any(handle))
        .route("/gw/{route_id}/{*rest}", any(handle))
        .route("/v1", any(unified))
        .route("/v1/{*rest}", any(unified))
        .layer(middleware::from_fn_with_state(state.clone(), auth))
        .layer(DefaultBodyLimit::max(DEFAULT_MAX_BODY))
        .with_state(state)
}

async fn health(AxumState(state): AxumState<Arc<State>>) -> Response {
    let snapshot = state.snapshot();
    (
        StatusCode::OK,
        axum::Json(json!({
            "ok": true,
            "routes": snapshot.routes().count(),
            "auth": snapshot.auth_token().is_some(),
            "version": env!("CARGO_PKG_VERSION"),
        })),
    )
        .into_response()
}

/// 从原始路径里取 route_id。
///
/// 不用 `Path<String>`：通配路由 `/gw/{route_id}/{*rest}` 有两个参数，
/// 单值提取器会在运行时 panic（500）。自己切一段既避免这个坑，
/// 也能让 `/gw/main` 与 `/gw/main/v1/messages` 走同一个 handler。
async fn handle(
    AxumState(state): AxumState<Arc<State>>,
    OriginalUri(uri): OriginalUri,
    method: Method,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let Some(route_id) = route_id_of(uri.path()) else {
        return proxy::error(StatusCode::NOT_FOUND, &t!(Msg::GatewayMissingRouteId));
    };
    proxy::handle(state, route_id, method, uri, headers, body).await
}

/// 统一入口：`Bearer <token>` 里的令牌就是供应商池的 route id。
///
/// 令牌**不是密钥** —— 它等于 agent 的 slug（`claude` / `codex` / …），
/// 本机任何进程都猜得到。这里承载的信息只是「该走哪个池」，真正的隔离靠
/// 只监听回环地址。所以 `/v1` 不走 `auth` 中间件的全局令牌校验。
async fn unified(
    AxumState(state): AxumState<Arc<State>>,
    OriginalUri(uri): OriginalUri,
    method: Method,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let Some(token) = presented_token(&headers) else {
        return unknown_token(&state, None);
    };
    let token = token.trim().to_string();
    if token.is_empty() {
        return unknown_token(&state, None);
    }
    if !state.snapshot().has_route(&token) {
        return unknown_token(&state, Some(&token));
    }
    proxy::handle(state, token, method, uri, headers, body).await
}

/// 令牌对不上时，把可用的池 id 一起返回 —— 用户一眼能看出自己该填什么。
fn unknown_token(state: &Arc<State>, presented: Option<&str>) -> Response {
    let tokens = state.snapshot().route_ids();
    let message = match presented {
        Some(token) => t!(
            Msg::GatewayUnknownToken,
            token = token,
            tokens = tokens.join(" / ")
        ),
        None => t!(Msg::GatewayMissingToken),
    };
    (
        StatusCode::UNAUTHORIZED,
        [(header::CONTENT_TYPE, "application/json")],
        json!({
            "error": { "type": "authentication_error", "message": message },
            "tokens": tokens,
        })
        .to_string(),
    )
        .into_response()
}

fn route_id_of(path: &str) -> Option<String> {
    let rest = path.strip_prefix("/gw/")?;
    let id = rest.split('/').next().unwrap_or("");
    if id.is_empty() {
        None
    } else {
        Some(id.to_string())
    }
}

/// 本地令牌校验。没配令牌时直接放行（只监听回环，风险可接受）。
///
/// `Authorization: Bearer <token>` 与 `x-api-key: <token>` 都认：
/// Claude Code 走前者，部分中转站 SDK 走后者。
async fn auth(AxumState(state): AxumState<Arc<State>>, req: Request, next: Next) -> Response {
    let path = req.uri().path();
    // `/v1` 的令牌是路由标识而不是密钥，由 `unified` 自己解析
    if path == "/healthz" || path == "/v1" || path.starts_with("/v1/") {
        return next.run(req).await;
    }
    let snapshot = state.snapshot();
    let Some(expected) = snapshot.auth_token().map(str::to_string) else {
        return next.run(req).await;
    };

    match presented_token(req.headers()) {
        Some(token) if token == expected => next.run(req).await,
        _ => (
            StatusCode::UNAUTHORIZED,
            [(header::CONTENT_TYPE, "application/json")],
            json!({
                "error": {
                    "type": "authentication_error",
                    "message": t!(Msg::GatewayTokenRequired),
                }
            })
            .to_string(),
        )
            .into_response(),
    }
}

fn presented_token(headers: &HeaderMap) -> Option<String> {
    if let Some(value) = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
    {
        let trimmed = value.trim();
        return Some(
            trimmed
                .strip_prefix("Bearer ")
                .unwrap_or(trimmed)
                .trim()
                .to_string(),
        );
    }
    headers
        .get("x-api-key")
        .and_then(|value| value.to_str().ok())
        .map(|value| value.trim().to_string())
}
