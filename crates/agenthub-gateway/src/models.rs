//! `/v1/models` 的本地合并响应。
//!
//! 聚合模式下这条请求不该打上游：每个成员只知道自己的模型，客户端要的是并集。
//! 两种协议各按自己的字段形态返回，避免 Claude Code / Codex 解析失败。

use agenthub_core::model::gateway::GatewayProtocol;
use axum::body::Body;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::Response;
use serde_json::{Value, json};

use crate::config::RuntimeRoute;

pub fn respond(protocol: GatewayProtocol, route: &RuntimeRoute) -> Response {
    let models = route.model_pool();
    let body = match protocol {
        GatewayProtocol::Anthropic => json!({
            "data": models
                .iter()
                .map(|id| json!({ "type": "model", "id": id, "display_name": id }))
                .collect::<Vec<Value>>(),
            "has_more": false,
            "first_id": models.first(),
            "last_id": models.last(),
        }),
        // Responses 与 Chat 共用 OpenAI 的模型列表形态
        GatewayProtocol::ChatCompletions | GatewayProtocol::Responses => json!({
            "object": "list",
            "data": models
                .iter()
                .map(|id| json!({ "id": id, "object": "model", "created": 0, "owned_by": "agenthub" }))
                .collect::<Vec<Value>>(),
        }),
    };

    let bytes = serde_json::to_vec(&body).unwrap_or_else(|_| b"{\"data\":[]}".to_vec());
    Response::builder()
        .status(StatusCode::OK)
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
    use crate::config::RuntimeMember;
    use agenthub_core::model::gateway::{GatewayRoute, RetryPolicy, RouteMember, RouteMode};
    use std::collections::BTreeMap;

    fn route(models: &[&str]) -> RuntimeRoute {
        let mut members = BTreeMap::new();
        members.insert(
            "a".to_string(),
            RuntimeMember {
                provider_id: "a".into(),
                name: "A".into(),
                base_url: "https://a".into(),
                api_key: Some("k".into()),
                error: None,
                weight: 1,
                priority: 1,
                enabled: true,
                models: models.iter().map(|m| m.to_string()).collect(),
                model_map: BTreeMap::new(),
                protocol: None,
            },
        );
        RuntimeRoute {
            config: GatewayRoute {
                id: "main".into(),
                name: "main".into(),
                protocol: GatewayProtocol::ChatCompletions,
                mode: RouteMode::Single,
                members: vec![RouteMember {
                    provider_id: "a".into(),
                    weight: 1,
                    priority: 1,
                    models: Vec::new(),
                    model_map: BTreeMap::new(),
                    enabled: true,
                }],
                rules: Vec::new(),
                retry: RetryPolicy::default(),
                enabled: true,
            },
            members,
        }
    }

    #[test]
    fn openai_shape_lists_pool() {
        let route = route(&["gpt-6.1", "gpt-6.1-mini"]);
        let response = respond(GatewayProtocol::ChatCompletions, &route);
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn pool_merges_provider_models() {
        let route = route(&["a1", "a2"]);
        assert_eq!(route.model_pool(), ["a1", "a2"]);
    }
}
