mod common;

use std::fs;

use agenthub_core::model::agent::{AgentId, Scope};
use agenthub_core::model::mcp::{McpServer, Transport};

fn sample() -> Vec<McpServer> {
    vec![McpServer {
        name: "chrome-devtools".into(),
        transport: Transport::Stdio {
            command: "npx".into(),
            args: vec!["-y".into(), "chrome-devtools-mcp@latest".into()],
            env: Default::default(),
            cwd: None,
        },
        enabled: true,
        timeout_secs: Some(120),
        note: None,
    }]
}

#[test]
fn mcp_write_three_formats() {
    let (home, data) = common::sandbox();
    common::setup(&home);
    let hub = common::hub(&home, &data);

    let servers = sample();
    let agents = vec![AgentId::Claude, AgentId::Codex, AgentId::Hermes];
    let plan = hub.plan_mcp_write(&servers, &agents, &Scope::User).unwrap();
    assert_eq!(plan.op_count(), 3, "三种格式各写一个文件");
    hub.apply(&plan, &Scope::User).unwrap();

    let claude: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(home.join(".claude.json")).unwrap()).unwrap();
    assert_eq!(claude["theme"], "dark", "原有键必须保留");
    assert_eq!(claude["mcpServers"]["chrome-devtools"]["command"], "npx");

    let codex = fs::read_to_string(home.join(".codex/config.toml")).unwrap();
    assert!(codex.contains("[model_providers.custom]"));
    assert!(codex.contains("base_url = \"https://example.com\""));
    assert!(codex.contains("[mcp_servers.chrome-devtools]"));
    assert!(codex.contains("startup_timeout_sec = 120"));

    let hermes = fs::read_to_string(home.join(".hermes/config.yaml")).unwrap();
    assert!(hermes.starts_with("# hermes"), "注释必须保留");
    assert!(hermes.contains("platform_toolsets:"));
    assert!(hermes.contains("chrome-devtools:"));
    assert!(!hermes.contains("  old:"), "旧条目应被整块替换");

    let views = hub.mcp_overview(&Scope::User).unwrap();
    let view = views
        .iter()
        .find(|v| v.name == "chrome-devtools")
        .expect("概览应包含 chrome-devtools");
    let present = view.targets.iter().filter(|t| t.present).count();
    assert_eq!(present, 3);

    // 幂等：再写一次不产生操作
    let again = hub.plan_mcp_write(&servers, &agents, &Scope::User).unwrap();
    assert!(again.is_empty(), "相同定义重复写入应为空计划");
}
