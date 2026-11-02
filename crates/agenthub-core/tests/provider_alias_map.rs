//! `model_map` 的模式语义：聚合/路由写映射，直连清空，切回重建。
//!
//! 单独一个测试文件：`common::sandbox()` 会设置一次全局路径覆盖，
//! 同文件里放两个用例并行跑会互相看见对方的 config。

mod common;

use std::collections::BTreeMap;

use agenthub_core::api::provider::ProviderInput;
use agenthub_core::model::gateway::{GatewayProtocol, PoolMode};

/// 切回直连必须把 `model_map` 清干净。
///
/// 以前的 `sync_alias_map` 在非网关模式下直接 `return`，于是直连之后池成员里还留着
/// 「官方名 → 上游名」的旧映射。界面上看不出来，但下一次切回网关时会先带着这份旧映射
/// 跑一轮——用户会以为映射是刚配的，实际是上一轮遗留的。
#[test]
fn direct_mode_clears_the_alias_map() {
    let (home, data) = common::sandbox();
    let claude_dir = home.join(".claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    std::fs::write(claude_dir.join("settings.json"), "{\"env\":{}}\n").unwrap();

    let mut hub = common::hub(&home, &data);
    hub.upsert_provider(ProviderInput {
        id: "relay".into(),
        name: "中转站".into(),
        base_url: "http://127.0.0.1:1/v1".into(),
        api_key: "sk-test".into(),
        models: vec!["m-fast".into(), "m-strong".into()],
        note: None,
        acknowledged: true,
        protocol: Some(GatewayProtocol::Anthropic),
        model_limits: BTreeMap::new(),
        catalog_id: None,
    })
    .unwrap();
    hub.add_to_pool("claude", "relay").unwrap();
    hub.set_model_slots(
        "claude",
        BTreeMap::from([
            ("sonnet".to_string(), "m-fast".to_string()),
            ("opus".to_string(), "m-strong".to_string()),
        ]),
    )
    .unwrap();

    let map_of = |hub: &agenthub_core::api::AgentHub| {
        hub.config
            .gateway_routes
            .iter()
            .find(|route| route.id == "claude")
            .and_then(|route| route.members.first())
            .map(|member| member.model_map.clone())
            .unwrap_or_default()
    };

    // 聚合：官方名 + [1M]/[1m] 变体都在
    hub.set_pool_mode("claude", PoolMode::Aggregate).unwrap();
    let aggregate = map_of(&hub);
    assert_eq!(
        aggregate.get("claude-sonnet-5").map(String::as_str),
        Some("m-fast"),
        "聚合模式要写官方名映射：{aggregate:?}"
    );
    assert!(
        aggregate.contains_key("claude-sonnet-5[1M]")
            && aggregate.contains_key("claude-sonnet-5[1m]"),
        "带 [1M] / [1m] 后缀的变体键也要写：{aggregate:?}"
    );

    // 删掉 opus 槽位：它的映射要回落到默认模型，而不是继续指向旧的 m-strong
    hub.set_model_slots(
        "claude",
        BTreeMap::from([("sonnet".to_string(), "m-fast".to_string())]),
    )
    .unwrap();
    let trimmed = map_of(&hub);
    assert_eq!(
        trimmed.get("claude-opus-5").map(String::as_str),
        Some("m-fast"),
        "删掉 opus 槽位后应回落到默认模型，实际 {trimmed:?}"
    );
    assert_eq!(
        trimmed.get("claude-opus-5[1M]").map(String::as_str),
        Some("m-fast"),
        "带后缀的变体也要跟着回落"
    );
    assert_eq!(
        trimmed.get("claude-sonnet-5").map(String::as_str),
        Some("m-fast")
    );

    // 直连：三种写法一起清掉，一个别名都不能留
    hub.set_pool_mode("claude", PoolMode::Direct).unwrap();
    let direct = map_of(&hub);
    assert!(
        direct.is_empty(),
        "切回直连后 model_map 必须清空，实际 {direct:?}"
    );

    // 把两个槽位配回来，再切回路由：映射按当前槽位重建（证明上一步只是清空，不是把能力关掉）
    hub.set_model_slots(
        "claude",
        BTreeMap::from([
            ("sonnet".to_string(), "m-fast".to_string()),
            ("opus".to_string(), "m-strong".to_string()),
        ]),
    )
    .unwrap();
    hub.set_pool_mode("claude", PoolMode::Route).unwrap();
    let route = map_of(&hub);
    assert_eq!(
        route.get("claude-opus-5").map(String::as_str),
        Some("m-strong"),
        "切回网关要重建映射：{route:?}"
    );
    assert_eq!(
        route.get("claude-sonnet-5").map(String::as_str),
        Some("m-fast")
    );
}
