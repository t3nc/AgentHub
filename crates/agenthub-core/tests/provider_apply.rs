//! 端到端：绑定 → 一键应用 → 读回真实文件，确认 OpenCode 真的能加载这个 provider。
//!
//! 这是「写了却不生效」那个 bug 的回归测试：只写 `options.baseURL` / `apiKey`
//! 时 OpenCode 不会加载它，必须有 `npm` 与 `models`。

mod common;

use std::collections::BTreeMap;

use agenthub_core::api::provider::{BindInput, ProviderInput};
use agenthub_core::ext::provider::{ModelLimitSource, ModelLimits};
use agenthub_core::model::gateway::GatewayProtocol;

#[test]
fn opencode_binding_writes_a_loadable_provider() {
    let (home, data) = common::sandbox();
    let opencode_dir = home.join(".config").join("opencode");
    std::fs::create_dir_all(&opencode_dir).unwrap();
    std::fs::write(
        opencode_dir.join("opencode.json"),
        concat!(
            "{\n",
            "  \"$schema\": \"https://opencode.ai/config.json\",\n",
            "  \"provider\": {\n",
            "    \"other\": {\n",
            "      \"npm\": \"@ai-sdk/openai-compatible\",\n",
            "      \"options\": { \"baseURL\": \"https://other.example.com/v1\" }\n",
            "    }\n",
            "  }\n",
            "}\n"
        ),
    )
    .unwrap();

    let mut hub = common::hub(&home, &data);
    hub.upsert_provider(ProviderInput {
        id: "agnes".into(),
        name: "Agnes".into(),
        // 端口 1 一定连不上：用来验证「写入成功但探测失败」不会回滚
        base_url: "http://127.0.0.1:1/v1".into(),
        api_key: "sk-test".into(),
        models: vec!["agnes-2.5-flash".into()],
        note: None,
        acknowledged: true,
        protocol: Some(GatewayProtocol::ChatCompletions),
        model_limits: BTreeMap::from([(
            "agnes-2.5-flash".to_string(),
            ModelLimits {
                context: Some(512_000),
                max_output: Some(65_536),
                source: ModelLimitSource::Auto,
            },
        )]),
        catalog_id: None,
    })
    .unwrap();

    hub.bind_provider(BindInput {
        agent: "opencode".into(),
        provider_id: "agnes".into(),
        model: Some("agnes-2.5-flash".into()),
        make_default: true,
        via_gateway: false,
        route_id: None,
        ..Default::default()
    })
    .unwrap();

    let report = hub.apply_provider_binding("opencode").unwrap();
    assert!(report.applied, "首次写入必须真的写盘：{report:?}");
    assert!(report.batch_id.is_some(), "写盘要留下可回滚的批次号");
    assert_eq!(report.files.len(), 1);
    let probe = report.probe.expect("应用后应探测一次");
    assert!(!probe.ok, "127.0.0.1:1 必然连不上，但配置仍然写成功了");

    let text = std::fs::read_to_string(opencode_dir.join("opencode.json")).unwrap();
    let doc: serde_json::Value = serde_json::from_str(&text).unwrap();
    let entry = &doc["provider"]["agnes"];

    // 这三项是 OpenCode 能加载 provider 的最低要求
    assert_eq!(entry["npm"], "@ai-sdk/openai-compatible");
    assert_eq!(entry["name"], "Agnes");
    assert_eq!(entry["options"]["baseURL"], "http://127.0.0.1:1/v1");
    assert_eq!(entry["options"]["apiKey"], "sk-test");
    assert_eq!(
        entry["models"]["agnes-2.5-flash"]["name"],
        "agnes-2.5-flash"
    );
    assert_eq!(
        entry["models"]["agnes-2.5-flash"]["options"]["contextWindow"],
        512_000
    );
    assert_eq!(
        entry["models"]["agnes-2.5-flash"]["options"]["maxOutputTokens"],
        65_536
    );

    // 别人的配置一个字都不能动
    assert_eq!(
        doc["provider"]["other"]["options"]["baseURL"],
        "https://other.example.com/v1"
    );
    assert_eq!(doc["$schema"], "https://opencode.ai/config.json");

    // 再点一次同样的配置：不该重复写盘，但仍然要探测一次
    let again = hub.apply_provider_binding("opencode").unwrap();
    assert!(!again.applied, "配置已是最新时不该再写一遍");
    assert!(again.skipped.is_some());
    assert!(again.probe.is_some(), "幂等点击也要给出连通状态");
}
