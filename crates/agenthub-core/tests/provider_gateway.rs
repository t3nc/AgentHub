//! 经网关写入的端到端：绑定 → 计划 → 回退计划 → 删路由退回直连。
//!
//! 单独成文件（而不是并进 `provider.rs`）：`paths::init_overrides` 是 `OnceLock`，
//! 一个测试进程只能钉死一次 home/data，同文件里放两个各自建沙箱的用例会互相污染。

mod common;

use std::collections::BTreeMap;

use agenthub_core::api::gateway::GatewayConfigInput;
use agenthub_core::api::provider::{BindInput, ProviderInput};
use agenthub_core::model::gateway::{
    GatewayProtocol, GatewayRoute, RetryPolicy, RouteMember, RouteMode,
};

fn provider_input() -> ProviderInput {
    ProviderInput {
        id: "relay".into(),
        name: "中转站".into(),
        base_url: "https://relay.example.com/v1".into(),
        // 用已确认的明文，避免测试依赖外部环境变量
        api_key: "sk-upstream-secret".into(),
        models: vec!["claude-sonnet-4-5".into()],
        note: None,
        acknowledged: true,
        protocol: None,
        model_limits: BTreeMap::new(),
        catalog_id: None,
    }
}

#[test]
fn gateway_binding_writes_local_url_and_never_the_upstream_key() {
    let (home, data) = common::sandbox();
    let claude_dir = home.join(".claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    std::fs::write(claude_dir.join("settings.json"), "{}\n").unwrap();

    let mut hub = common::hub(&home, &data);
    hub.upsert_provider(provider_input()).unwrap();

    hub.save_gateway_route(GatewayRoute {
        id: "claude-main".into(),
        name: "主链路".into(),
        protocol: GatewayProtocol::Anthropic,
        mode: RouteMode::Single,
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
    })
    .unwrap();

    hub.save_gateway_config(&GatewayConfigInput {
        port: 43117,
        auto_start: false,
        log_requests: false,
        max_log_entries: 500,
        auth_token: "tk-local".into(),
    })
    .unwrap();

    hub.bind_provider(BindInput {
        agent: "claude".into(),
        provider_id: "relay".into(),
        model: Some("claude-sonnet-4-5".into()),
        make_default: false,
        via_gateway: true,
        route_id: Some("claude-main".into()),
        ..Default::default()
    })
    .unwrap();

    // 1. 计划里是本地地址 + 本地令牌，上游密钥一次都不出现
    let cs = hub.plan_provider_apply("claude").unwrap();
    assert_eq!(cs.ops.len(), 1);
    let rendered = serde_json::to_string(&cs.ops[0]).unwrap();
    assert!(
        rendered.contains("http://127.0.0.1:43117/v1"),
        "经网关时 base_url 应指向统一入口 /v1：{rendered}"
    );
    // 令牌写在 env.ANTHROPIC_AUTH_TOKEN 里；要断言具体值就得看 content 本身
    // （FileOp 的 JSON 会把引号转义，直接 contains("\"claude\"") 匹配不到）
    let content = match &cs.ops[0].action {
        agenthub_core::model::change::OpAction::WriteFile { content, .. } => content.clone(),
        other => panic!("期望是写文件操作，实际是 {other:?}"),
    };
    assert!(
        content.contains("\"ANTHROPIC_AUTH_TOKEN\": \"claude\""),
        "网关模式的令牌就是 agent 名，网关靠它选中供应商池：{content}"
    );
    assert!(
        !rendered.contains("sk-upstream-secret"),
        "上游密钥不该出现在 agent 配置里：{rendered}"
    );

    // 2. 回退计划写回上游地址与上游密钥
    let revert = hub.plan_revert_to_direct(&["claude".to_string()]).unwrap();
    assert_eq!(revert.ops.len(), 1);
    let reverted = serde_json::to_string(&revert.ops[0]).unwrap();
    assert!(
        reverted.contains("https://relay.example.com/v1"),
        "回退计划应写回上游 base_url：{reverted}"
    );
    assert!(
        reverted.contains("sk-upstream-secret"),
        "回退后必须重新带上上游密钥，否则 agent 连不上：{reverted}"
    );

    // 3. 删路由：绑定退回直连，不留悬空状态
    let result = hub.delete_gateway_route("claude-main").unwrap();
    assert_eq!(result.reverted, vec!["claude".to_string()]);
    let binding = hub.config.provider_bindings.get("claude").unwrap();
    assert!(!binding.via_gateway, "删除路由后绑定应退回直连");
    assert!(binding.route_id.is_none());

    // 4. 功能开关：关掉 Provider 后写入被拒绝，且提示去哪儿开
    hub.set_feature("provider", false).unwrap();
    let err = hub.plan_provider_apply("claude").unwrap_err();
    // 文案按当前界面语言取词，断言不能钉死某一种语言：这里只验证「说清了禁用 + 去哪儿开」
    let message = err.to_string();
    let mentions_disabled =
        message.contains("已禁用") || message.to_lowercase().contains("disabled");
    let mentions_where = message.contains("功能管理") || message.contains("Features");
    assert!(
        mentions_disabled && mentions_where,
        "禁用后应返回可操作的错误：{message}"
    );
}
