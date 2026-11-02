//! 供应商池的端到端行为：加入 → 建池路由 → 切换三模式 → 写盘内容正确。
//!
//! 这是「点一下就换」那条链路的核心断言：每一步都要真的落进 agent 配置文件，
//! 而不是只改 config.json。

mod common;

use std::collections::BTreeMap;

use agenthub_core::api::provider::ProviderInput;
use agenthub_core::model::gateway::{GatewayProtocol, PoolMode};

fn claude_settings(home: &std::path::Path) -> serde_json::Value {
    let text = std::fs::read_to_string(home.join(".claude").join("settings.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

#[test]
fn pool_lifecycle_writes_the_expected_config() {
    let (home, data) = common::sandbox();
    let claude_dir = home.join(".claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    // 先放一个别的工具写的残留：NAME 键必须被清掉，其它键必须留着
    std::fs::write(
        claude_dir.join("settings.json"),
        concat!(
            "{\n",
            "  \"env\": {\n",
            "    \"ANTHROPIC_DEFAULT_OPUS_MODEL_NAME\": \"别的工具写的显示名\",\n",
            "    \"KEEP_ME\": \"yes\"\n",
            "  },\n",
            "  \"permissions\": { \"allow\": [] }\n",
            "}\n"
        ),
    )
    .unwrap();

    let mut hub = common::hub(&home, &data);
    hub.upsert_provider(ProviderInput {
        id: "relay".into(),
        name: "中转站".into(),
        // 端口 1 必然连不上：只验证写盘，不等超时
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

    // 1. 加入池：池原本为空 → 自动成为默认，路由 id = claude
    let report = hub.add_to_pool("claude", "relay").unwrap();
    assert!(report.applied, "加入池应真的写盘：{report:?}");
    assert!(
        hub.config.gateway_routes.iter().any(|r| r.id == "claude"),
        "应自动建一条 id = claude 的池路由"
    );
    let binding = hub.config.provider_bindings.get("claude").unwrap();
    assert_eq!(binding.provider_id, "relay");
    assert_eq!(binding.mode, PoolMode::Direct, "默认从直连开始");

    let doc = claude_settings(&home);
    assert_eq!(
        doc["env"]["ANTHROPIC_BASE_URL"], "http://127.0.0.1:1/v1",
        "直连应写上游地址"
    );
    assert_eq!(doc["env"]["KEEP_ME"], "yes", "无关的键不能动");
    assert_eq!(doc["permissions"]["allow"], serde_json::json!([]));

    // 2. 配置模型槽位：sonnet 与 opus 指向不同上游模型
    let slots = BTreeMap::from([
        ("sonnet".to_string(), "m-fast".to_string()),
        ("opus".to_string(), "m-strong".to_string()),
    ]);
    hub.set_model_slots("claude", slots.clone()).unwrap();
    let doc = claude_settings(&home);
    assert_eq!(
        doc["env"]["ANTHROPIC_DEFAULT_SONNET_MODEL"], "m-fast",
        "直连时别名直接写上游模型名"
    );
    assert_eq!(doc["env"]["ANTHROPIC_DEFAULT_OPUS_MODEL"], "m-strong");
    assert!(
        doc["env"]
            .get("ANTHROPIC_DEFAULT_OPUS_MODEL_NAME")
            .is_none(),
        "会误导的显示名必须清掉"
    );

    // 3. 切聚合：写统一入口 + 令牌，别名改成官方名，映射进池成员
    hub.set_pool_mode("claude", PoolMode::Aggregate).unwrap();
    let doc = claude_settings(&home);
    assert_eq!(
        doc["env"]["ANTHROPIC_BASE_URL"], "http://127.0.0.1:43117/v1",
        "网关模式应写统一入口"
    );
    assert_eq!(
        doc["env"]["ANTHROPIC_AUTH_TOKEN"], "claude",
        "令牌 = agent 名"
    );
    assert_eq!(
        doc["env"]["ANTHROPIC_DEFAULT_SONNET_MODEL"], "claude-sonnet-5",
        "网关模式写官方别名"
    );
    assert_eq!(doc["env"]["ANTHROPIC_DEFAULT_OPUS_MODEL"], "claude-opus-5");

    let route = hub
        .config
        .gateway_routes
        .iter()
        .find(|r| r.id == "claude")
        .unwrap();
    assert!(route.enabled, "聚合模式下池路由要启用，否则 /v1 找不到它");
    let member = &route.members[0];
    assert_eq!(
        member.model_map.get("claude-sonnet-5").map(String::as_str),
        Some("m-fast"),
        "官方名必须映射到槽位指定的上游模型"
    );
    assert_eq!(
        member.model_map.get("claude-opus-5").map(String::as_str),
        Some("m-strong")
    );
    assert_eq!(
        member
            .model_map
            .get("claude-sonnet-5[1M]")
            .map(String::as_str),
        Some("m-fast"),
        "带后缀的请求也要能命中"
    );

    // 4. 切回直连：地址回到上游，池路由停用但保留
    hub.set_pool_mode("claude", PoolMode::Direct).unwrap();
    let doc = claude_settings(&home);
    assert_eq!(doc["env"]["ANTHROPIC_BASE_URL"], "http://127.0.0.1:1/v1");
    assert_eq!(doc["env"]["ANTHROPIC_DEFAULT_SONNET_MODEL"], "m-fast");
    let route = hub
        .config
        .gateway_routes
        .iter()
        .find(|r| r.id == "claude")
        .unwrap();
    assert!(!route.enabled, "直连时池路由停用");
    assert_eq!(route.members.len(), 1, "但成员要留着，切回来不用重加");

    // 5. 移除最后一个成员：绑定一起清掉，不留悬空
    hub.remove_from_pool("claude", "relay").unwrap();
    assert!(!hub.config.provider_bindings.contains_key("claude"));
}
