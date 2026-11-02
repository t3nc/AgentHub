//! 转发主管线：候选顺序 → 逐候选请求 → 首字节前重试 → 流式回写 → 记一行日志。

use agenthub_core::l10n::{Msg, t};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri, header};
use axum::response::Response;
use bytes::Bytes;
use futures_util::StreamExt;
use serde_json::json;
use tokio::sync::mpsc::UnboundedSender;

use agenthub_core::ext::gateway as gateway_logic;

use crate::State;
use crate::convert;
use crate::protocol::{CanonicalRequest, adapter_for};
use crate::upstream;

/// 一次成功/失败的请求元数据。
///
/// **不含请求体与响应体**：prompt 里可能有代码和密钥，落日志等于把它们抄一遍。
#[derive(Debug, Clone, serde::Serialize)]
pub struct RequestLog {
    /// Unix 秒
    pub at: i64,
    pub route_id: String,
    pub provider_id: Option<String>,
    pub provider_name: Option<String>,
    pub model: Option<String>,
    pub protocol: String,
    /// 从 User-Agent 猜出来的来源 CLI（claude / codex / …）
    pub agent: Option<String>,
    pub status: u16,
    pub attempts: u32,
    pub first_byte_ms: u64,
    pub total_ms: u64,
    pub bytes: u64,
    pub error: Option<String>,
}

/// 响应体消费完（或连接断开）时把日志补全并发出。
///
/// 放在 `Drop` 上是因为流式场景只有读完才知道总耗时与总字节数；用一个独立的
/// 统计 task 反而会在连接断开时泄漏。
struct LogGuard {
    tx: UnboundedSender<RequestLog>,
    log: RequestLog,
    bytes: Arc<AtomicU64>,
    started: Instant,
}

impl Drop for LogGuard {
    fn drop(&mut self) {
        let mut log = self.log.clone();
        log.total_ms = self.started.elapsed().as_millis() as u64;
        log.bytes = self.bytes.load(Ordering::Relaxed);
        let _ = self.tx.send(log);
    }
}

pub async fn handle(
    state: Arc<State>,
    route_id: String,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let started = Instant::now();
    let snapshot = state.snapshot();
    let Some(route) = snapshot.route(&route_id) else {
        return error(
            StatusCode::NOT_FOUND,
            &t!(Msg::GatewayRouteUnknown, route = route_id),
        );
    };
    if !route.config.enabled {
        return error(
            StatusCode::SERVICE_UNAVAILABLE,
            &t!(Msg::GatewayRouteDisabled),
        );
    }

    let path = strip_prefix(uri.path(), &route_id);
    let request = CanonicalRequest::parse(
        route.config.protocol,
        method.as_str(),
        &path,
        uri.query(),
        body,
    );

    // 模型列表由网关本地合并：聚合模式下每个成员只知道自己的模型
    if request.is_models() {
        return crate::models::respond(route.config.protocol, route);
    }

    let tick = state.tick.fetch_add(1, Ordering::Relaxed);
    let candidates = snapshot.candidates(route, request.model.as_deref(), tick);
    if candidates.is_empty() {
        return error(StatusCode::BAD_GATEWAY, &t!(Msg::GatewayNoUpstream));
    }

    let policy = route.config.retry.normalized();
    let agent = agent_of(&headers);
    let mut attempts = 0u32;
    let mut first_byte_ms = 0u64;
    let mut last_error: Option<String> = None;
    let mut last_status = StatusCode::BAD_GATEWAY;

    // 总尝试次数由策略决定；成员不够时回到队首重试同一个上游（单上游透传也就能重试），
    // 成员足够时就是「换下一个」。
    let total_attempts = policy.max_attempts.max(1) as usize;
    for index in 0..total_attempts {
        let candidate = &candidates[index % candidates.len()];
        attempts += 1;
        let Some(api_key) = candidate.api_key.as_deref() else {
            last_error = candidate.error.clone();
            continue;
        };

        // 上游协议：供应商没声明时跟随路由（旧行为，字节级透传）
        let upstream_protocol =
            gateway_logic::upstream_protocol(route.config.protocol, candidate.protocol);
        let conversion = convert::direction(route.config.protocol, upstream_protocol);

        let (path, payload, extra) = match conversion {
            Some(direction) => {
                // 跨协议：请求体要重写成上游认识的形态，路径也跟着换
                let value: serde_json::Value =
                    serde_json::from_slice(&request.body).unwrap_or_else(|_| serde_json::json!({}));
                let model = candidate
                    .model_map
                    .get(request.model.as_deref().unwrap_or_default())
                    .cloned()
                    .or_else(|| request.model.clone())
                    .unwrap_or_default();
                let converted = convert::convert_request(direction, &value, &model);
                (
                    convert::upstream_path(direction).to_string(),
                    Bytes::from(serde_json::to_vec(&converted).unwrap_or_default()),
                    Vec::new(),
                )
            }
            None => {
                let (payload, extra) =
                    adapter_for(route.config.protocol).prepare(&request, &candidate.model_map);
                (request.path.clone(), payload, extra)
            }
        };

        let url = upstream::upstream_url(&candidate.base_url, &path, request.query.as_deref());
        let mut out_headers = upstream::request_headers(&headers, upstream_protocol, Some(api_key));
        for (name, value) in extra {
            if let (Ok(name), Ok(value)) = (
                HeaderName::try_from(name.as_str()),
                HeaderValue::from_str(&value),
            ) {
                out_headers.insert(name, value);
            }
        }

        let mut builder = state
            .client
            .request(method.clone(), &url)
            .headers(out_headers);
        if !payload.is_empty() {
            builder = builder.body(payload);
        }

        let attempt_started = Instant::now();
        let sent =
            tokio::time::timeout(Duration::from_millis(policy.first_byte_ms), builder.send()).await;

        let response = match sent {
            Err(_) => {
                last_error = Some(t!(
                    Msg::GatewayFirstByteTimeout,
                    name = candidate.name,
                    ms = policy.first_byte_ms
                ));
                continue;
            }
            Ok(Err(err)) => {
                last_error = Some(t!(
                    Msg::GatewayConnectFailed,
                    name = candidate.name,
                    error = err
                ));
                continue;
            }
            Ok(Ok(response)) => response,
        };

        let status = response.status();
        first_byte_ms = attempt_started.elapsed().as_millis() as u64;

        let retryable = policy.should_retry_status(status.as_u16());
        let has_next = index + 1 < total_attempts;
        if retryable && has_next {
            last_status = status;
            last_error = Some(t!(
                Msg::GatewayUpstreamStatus,
                name = candidate.name,
                status = status
            ));
            // 退避一点再试：上游 429 时立刻重打往往还是 429
            tokio::time::sleep(Duration::from_millis(150 * attempts as u64)).await;
            continue;
        }

        let log = RequestLog {
            at: chrono_now(),
            route_id: route.config.id.clone(),
            provider_id: Some(candidate.provider_id.clone()),
            provider_name: Some(candidate.name.clone()),
            model: request.model.clone(),
            protocol: route.config.protocol.label().to_string(),
            agent,
            status: status.as_u16(),
            attempts,
            first_byte_ms,
            total_ms: 0,
            bytes: 0,
            error: None,
        };
        return stream(state, response, log, started, conversion, request.stream).await;
    }

    // 全部候选都失败：把最后一次状态码带回去，便于客户端按语义重试
    let log = RequestLog {
        at: chrono_now(),
        route_id: route.config.id.clone(),
        provider_id: None,
        provider_name: None,
        model: request.model.clone(),
        protocol: route.config.protocol.label().to_string(),
        agent,
        status: last_status.as_u16(),
        attempts,
        first_byte_ms,
        total_ms: started.elapsed().as_millis() as u64,
        bytes: 0,
        error: last_error.clone(),
    };
    let _ = state.logs.send(log);

    let message = last_error.unwrap_or_else(|| t!(Msg::GatewayAllUpstreamsFailed));
    error(StatusCode::BAD_GATEWAY, &message)
}

/// 回写响应。
///
/// 三种路径：
/// - 同协议 + 流式：逐块透传（不缓冲，SSE 一旦攒起来 agent 就会等到超时）；
/// - 跨协议 + 流式：逐块喂进转换状态机再写出；
/// - 跨协议 + 非流式：整包读完再转换（非流式本来就没有增量语义）。
async fn stream(
    state: Arc<State>,
    response: reqwest::Response,
    log: RequestLog,
    started: Instant,
    conversion: Option<convert::Direction>,
    wants_stream: bool,
) -> Response {
    let status = response.status();
    let mut headers = upstream::response_headers(response.headers());

    if let Some(direction) = conversion {
        if !wants_stream || !status.is_success() {
            // 错误响应原样透传：客户端看到上游的原始错误比看到一份转换失败的 JSON 更有用
            return convert_body(state, response, log, started, direction, status, headers).await;
        }
        // 转换后的分块不再对应上游的 content-type / 长度
        headers.remove(header::CONTENT_LENGTH);
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/event-stream"),
        );

        let bytes = Arc::new(AtomicU64::new(0));
        let counter = bytes.clone();
        let guard = LogGuard {
            tx: state.logs.clone(),
            log,
            bytes: bytes.clone(),
            started,
        };
        let bridged = convert::stream_bridge::bridge(Box::pin(response.bytes_stream()), direction);
        let body_stream = bridged.map(move |item| {
            if let Ok(ref data) = item {
                counter.fetch_add(data.len() as u64, Ordering::Relaxed);
            }
            let _ = &guard;
            item
        });
        return build_response(status, headers, Body::from_stream(body_stream));
    }

    let bytes = Arc::new(AtomicU64::new(0));
    let counter = bytes.clone();
    let guard = LogGuard {
        tx: state.logs.clone(),
        log,
        bytes: bytes.clone(),
        started,
    };

    let body_stream = response.bytes_stream().map(move |chunk| {
        if let Ok(ref data) = chunk {
            counter.fetch_add(data.len() as u64, Ordering::Relaxed);
        }
        // 让 guard 活到流被 drop 的那一刻
        let _ = &guard;
        chunk
    });

    build_response(status, headers, Body::from_stream(body_stream))
}

/// 非流式响应：读完整包，转成客户端协议再写回。
async fn convert_body(
    state: Arc<State>,
    response: reqwest::Response,
    log: RequestLog,
    started: Instant,
    direction: convert::Direction,
    status: StatusCode,
    mut headers: axum::http::HeaderMap,
) -> Response {
    let raw = match response.bytes().await {
        Ok(bytes) => bytes,
        Err(err) => {
            let mut log = log;
            log.error = Some(t!(Msg::GatewayReadUpstreamFailed, error = err));
            log.total_ms = started.elapsed().as_millis() as u64;
            let _ = state.logs.send(log);
            return error(
                StatusCode::BAD_GATEWAY,
                &t!(Msg::GatewayReadUpstreamFailedShort),
            );
        }
    };

    let body = match serde_json::from_slice::<serde_json::Value>(&raw) {
        Ok(value) if status.is_success() => {
            let converted = convert::convert_response(direction, &value);
            serde_json::to_vec(&converted).unwrap_or_else(|_| raw.to_vec())
        }
        // 非 JSON（错误页 / HTML）原样返回
        _ => raw.to_vec(),
    };

    headers.remove(header::CONTENT_LENGTH);
    if !headers.contains_key(header::CONTENT_TYPE) {
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
    }

    let mut log = log;
    log.total_ms = started.elapsed().as_millis() as u64;
    log.bytes = body.len() as u64;
    let _ = state.logs.send(log);

    build_response(status, headers, Body::from(body))
}

fn build_response(status: StatusCode, headers: axum::http::HeaderMap, body: Body) -> Response {
    let mut builder = Response::builder().status(status);
    if let Some(out) = builder.headers_mut() {
        for (name, value) in headers.iter() {
            out.append(name.clone(), value.clone());
        }
        if !out.contains_key(header::CONTENT_TYPE) {
            out.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
        }
    }
    builder.body(body).unwrap_or_else(|_| {
        error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &t!(Msg::GatewayResponseBuildFailed),
        )
    })
}

fn strip_prefix(path: &str, route_id: &str) -> String {
    let prefix = format!("/gw/{route_id}");
    match path.strip_prefix(&prefix) {
        Some("") => "/".to_string(),
        // 必须整段匹配：`/gw/mainx/v1` 不能被当成 `main` 的子路径
        Some(rest) if rest.starts_with('/') => rest.to_string(),
        _ => path.to_string(),
    }
}

fn agent_of(headers: &HeaderMap) -> Option<String> {
    let raw = headers
        .get(header::USER_AGENT)?
        .to_str()
        .ok()?
        .to_ascii_lowercase();
    for (needle, slug) in [
        ("claude", "claude"),
        ("codex", "codex"),
        ("codebuddy", "codebuddy"),
        ("gemini", "gemini"),
        ("opencode", "opencode"),
        ("hermes", "hermes"),
        ("workbuddy", "workbuddy"),
    ] {
        if raw.contains(needle) {
            return Some(slug.to_string());
        }
    }
    None
}

fn chrono_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn error(status: StatusCode, message: &str) -> Response {
    let body = json!({
        "error": {
            "type": "agenthub_gateway_error",
            "message": message,
        }
    });
    let bytes = serde_json::to_vec(&body).unwrap_or_default();
    Response::builder()
        .status(status)
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        )
        .body(Body::from(bytes))
        .unwrap_or_else(|_| Response::new(Body::empty()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_route_prefix() {
        assert_eq!(strip_prefix("/gw/main/v1/messages", "main"), "/v1/messages");
        assert_eq!(strip_prefix("/gw/main", "main"), "/");
        assert_eq!(strip_prefix("/gw/mainx/v1", "main"), "/gw/mainx/v1");
    }

    #[test]
    fn detects_agent_from_user_agent() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::USER_AGENT,
            HeaderValue::from_static("claude-cli/2.0.1"),
        );
        assert_eq!(agent_of(&headers).as_deref(), Some("claude"));
        headers.insert(
            header::USER_AGENT,
            HeaderValue::from_static("codex_cli_rs/0.9"),
        );
        assert_eq!(agent_of(&headers).as_deref(), Some("codex"));
        headers.insert(header::USER_AGENT, HeaderValue::from_static("curl/8"));
        assert_eq!(agent_of(&headers), None);
    }
}
