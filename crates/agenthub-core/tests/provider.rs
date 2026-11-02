//! Phase 3（Provider）端到端：登记 → 绑定 → 计划 → 落盘 → 回滚。
//!
//! 全程在沙箱 `AGENTHUB_HOME` 里跑，**不碰**真实的 `~/.claude/settings.json` 与
//! `~/.codex/config.toml`。合并细节（BOM / EOL / 注释保留）由 `ext::provider` 的
//! 单元测试覆盖，这里只验编排链路与安全边界。

mod common;

use std::collections::BTreeMap;

use agenthub_core::api::provider::{BindInput, ProviderInput};
use agenthub_core::api::settings::SettingsInput;
use agenthub_core::l10n::{Locale, Msg, text_for};
use agenthub_core::model::agent::Scope;
use agenthub_core::model::change::RiskLevel;

/// 同一句文案在三语下的全部写法。
///
/// 测试不许钉死某一种语言（全局 locale 与 `cargo test` 并行冲突，同一个测试二进制里
/// 任一用例改过语言就会影响其它用例），所以「断言某条错误」一律用这个集合比对。
fn tri(msg: Msg) -> Vec<String> {
    Locale::ALL
        .iter()
        .map(|locale| text_for(*locale, msg, &[]))
        .collect()
}

fn provider_input(id: &str, api_key: &str, acknowledged: bool) -> ProviderInput {
    ProviderInput {
        id: id.into(),
        name: format!("{id} 中转站"),
        base_url: "https://relay.example.com/v1".into(),
        api_key: api_key.into(),
        models: Vec::new(),
        note: None,
        acknowledged,
        protocol: None,
        model_limits: BTreeMap::new(),
        catalog_id: None,
    }
}

#[test]
fn provider_flow_merges_backs_up_and_rolls_back() {
    let (home, data) = common::sandbox();

    // 真实场景里这两个文件里还有用户自己的内容
    let claude_dir = home.join(".claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    let claude_file = claude_dir.join("settings.json");
    let claude_original = "{\n  \"hooks\": {\"Stop\": []},\n  \"env\": {\"KEEP\": \"1\"}\n}\n";
    std::fs::write(&claude_file, claude_original).unwrap();

    let codex_dir = home.join(".codex");
    std::fs::create_dir_all(&codex_dir).unwrap();
    let codex_file = codex_dir.join("config.toml");
    let codex_original = concat!(
        "# 我的 codex 配置\n",
        "model = \"gpt-6.1\"\n",
        "\n",
        "[model_providers.official]\n",
        "name = \"official\"\n",
        "base_url = \"https://api.openai.com/v1\"\n",
    );
    std::fs::write(&codex_file, codex_original).unwrap();

    let mut hub = common::hub(&home, &data);

    // 1. 明文密钥没勾确认 → 拒绝
    let err = hub
        .upsert_provider(provider_input("plain", "sk-plain-key", false))
        .unwrap_err();
    let expected = tri(Msg::ErrLiteralKeyUnacknowledged);
    assert!(
        expected.iter().any(|text| text == &err.to_string()),
        "应提示需要确认，实际：{err}"
    );

    // 2. 登记成功，且 DTO 里只有脱敏形式
    let view = hub
        .upsert_provider(provider_input("relay", "sk-plain-key", true))
        .unwrap();
    assert_eq!(view.providers.len(), 1);
    assert_eq!(view.providers[0].api_key, "literal:********");
    assert!(
        !serde_json::to_string(&view)
            .unwrap()
            .contains("sk-plain-key"),
        "视图里不能出现明文"
    );

    // 3. 绑定 Claude Code；Gemini 明确拒绝
    hub.bind_provider(BindInput {
        agent: "claude".into(),
        provider_id: "relay".into(),
        model: Some("claude-sonnet-4-5".into()),
        make_default: true,
        via_gateway: false,
        route_id: None,
        ..Default::default()
    })
    .unwrap();
    let unsupported = hub.bind_provider(BindInput {
        agent: "gemini".into(),
        provider_id: "relay".into(),
        model: None,
        make_default: false,
        via_gateway: false,
        route_id: None,
        ..Default::default()
    });
    assert!(unsupported.is_err());

    // 4. 生成计划：一个写操作 + 明文密钥与「设为默认」两条风险
    let cs = hub.plan_provider_apply("claude").unwrap();
    assert_eq!(cs.ops.len(), 1);
    let literal_key_risk = tri(Msg::RiskProviderLiteralKey);
    assert!(
        cs.risks.iter().any(|risk| risk.level == RiskLevel::Warn
            && literal_key_risk.iter().any(|text| text == &risk.message)),
        "应提示明文密钥风险，实际 {:?}",
        cs.risks
    );

    // 5. 落盘后原有键仍在，新键写进去
    let result = hub.apply(&cs, &Scope::User).unwrap();
    assert!(result.backup_dir.is_some(), "改写配置前必须留备份");
    let merged = std::fs::read_to_string(&claude_file).unwrap();
    let doc: serde_json::Value = serde_json::from_str(&merged).unwrap();
    assert_eq!(
        doc["hooks"]["Stop"],
        serde_json::json!([]),
        "hooks 必须保留"
    );
    assert_eq!(
        doc["env"]["KEEP"],
        serde_json::json!("1"),
        "已有 env 必须保留"
    );
    assert_eq!(
        doc["env"]["ANTHROPIC_BASE_URL"],
        serde_json::json!("https://relay.example.com/v1")
    );
    assert_eq!(
        doc["env"]["ANTHROPIC_AUTH_TOKEN"],
        serde_json::json!("sk-plain-key")
    );
    assert_eq!(doc["model"], serde_json::json!("claude-sonnet-4-5"));

    // 6. Codex 只接受 env: 引用：keychain 在**绑定**阶段就拦下（以前是 plan 阶段才报错，
    //    config.json 里会留下一条永远无法应用的悬空绑定）
    hub.upsert_provider(provider_input("kc", "keychain:agenthub/relay", false))
        .unwrap();
    let rejected = hub.bind_provider(BindInput {
        agent: "codex".into(),
        provider_id: "kc".into(),
        model: Some("gpt-6.1".into()),
        make_default: false,
        via_gateway: false,
        route_id: None,
        ..Default::default()
    });
    assert!(rejected.is_err(), "Codex 绑不了 keychain 引用");
    assert!(
        hub.config
            .provider_bindings
            .get("codex")
            .map(|b| b.provider_id != "kc")
            .unwrap_or(true),
        "被拒绝的绑定不能留在 config 里"
    );
    assert_eq!(
        std::fs::read_to_string(&codex_file).unwrap(),
        codex_original,
        "绑定失败不能动文件"
    );

    // 7. 换成 env: 引用后成功，且只写变量名
    hub.upsert_provider(provider_input("envrelay", "env:RELAY_API_KEY", false))
        .unwrap();
    hub.bind_provider(BindInput {
        agent: "codex".into(),
        provider_id: "envrelay".into(),
        model: Some("gpt-6.1".into()),
        make_default: true,
        via_gateway: false,
        route_id: None,
        ..Default::default()
    })
    .unwrap();
    let codex_plan = hub.plan_provider_apply("codex").unwrap();
    assert_eq!(codex_plan.ops.len(), 1);
    hub.apply(&codex_plan, &Scope::User).unwrap();
    let text = std::fs::read_to_string(&codex_file).unwrap();
    assert!(text.contains("# 我的 codex 配置"), "注释必须保留");
    assert!(
        text.contains("[model_providers.official]"),
        "其它 provider 必须保留"
    );
    assert!(text.contains("env_key = \"RELAY_API_KEY\""), "只写变量名");
    assert!(text.contains("model_provider = \"envrelay\""));
    assert!(!text.contains("keychain:"), "不能把引用当值写进去");

    // 8. 再生成一次应为空计划（幂等）
    let again = hub.plan_provider_apply("codex").unwrap();
    assert!(again.ops.is_empty(), "配置已是最新时不该再写");

    // 9. 保存设置不会清空 Provider 注册表与绑定（build_config 是 clone 后覆盖白名单）
    let before = hub.settings_view();
    let input = SettingsInput {
        store_root: before.store_root.clone(),
        default_strategy: before.default_strategy,
        backups_keep: before.backups_keep,
        excluded_paths: before.excluded_paths.clone(),
        use_trash_for_dedupe: before.use_trash_for_dedupe,
        backup_config_files: before.backup_config_files,
        agent_paths: BTreeMap::new(),
        custom_agents: Vec::new(),
        market_source: String::new(),
        market_registry: String::new(),
    };
    let after = hub.save_settings(input).unwrap();
    assert_eq!(
        after.providers.providers.len(),
        before.providers.providers.len(),
        "保存设置不能丢掉 provider"
    );
    assert!(
        after
            .providers
            .bindings
            .iter()
            .any(|binding| binding.agent == "codex" && binding.provider_id.is_some()),
        "绑定也要保留"
    );

    // 10. 回滚第一个批次：Claude 配置逐字节还原
    hub.rollback(&result.batch_id).unwrap();
    assert_eq!(
        std::fs::read_to_string(&claude_file).unwrap(),
        claude_original
    );

    // 11. 删除 provider 会同时摘掉绑定
    let view = hub.delete_provider("envrelay").unwrap();
    assert!(view.providers.iter().all(|item| item.id != "envrelay"));
    assert!(
        view.bindings
            .iter()
            .filter(|binding| binding.agent == "codex")
            .all(|binding| binding.provider_id.is_none()),
        "悬空绑定必须清掉"
    );
}
