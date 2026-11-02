//! 自定义平台（`custom_agents`）的 MCP 分发回归。
//!
//! 2026-11-02 macOS 实测发现两个缺陷，这里把它们钉住：
//! 1. 适配器身份曾用 `display_name`（`custom:Trae`），而 CLI / 前端传的是登记 key
//!    （`custom:trae`），`plan_mcp_write` 按 id 过滤时一个都对不上，计划恒为 0 个操作；
//! 2. Trae 与 Trae CN 的项目级 MCP 是同一个 `.trae/mcp.json`，两版都登记时必须只写一次。
//!
//! 两段断言写在**同一个用例**里：`paths::init_overrides` 是进程级 `OnceLock`，
//! 同一文件里放两个各自建沙箱的 `#[test]`，并行跑时会互相看见对方的 home/data
//! （见 `tests/common/mod.rs` 顶部的说明）。

mod common;

use std::fs;

use agenthub_core::AgentHub;
use agenthub_core::AppConfig;
use agenthub_core::config::CustomAgentDef;
use agenthub_core::model::agent::{AgentId, Scope};
use agenthub_core::model::mcp::{McpFormat, McpServer, Transport};

fn custom(id: &str, name: &str) -> CustomAgentDef {
    CustomAgentDef {
        id: id.to_string(),
        display_name: name.to_string(),
        skills_dir: None,
        project_skills_dir: None,
        mcp_file: None,
        project_mcp_file: Some(".trae/mcp.json".to_string()),
        mcp_format: Some(McpFormat::Json {
            pointer: vec!["mcpServers".to_string()],
        }),
        instructions: Vec::new(),
    }
}

fn demo_server() -> McpServer {
    McpServer {
        name: "demo".to_string(),
        transport: Transport::Stdio {
            command: "echo".to_string(),
            args: vec!["hi".to_string()],
            env: Default::default(),
            cwd: None,
        },
        enabled: true,
        timeout_secs: None,
        note: None,
    }
}

#[test]
fn custom_agent_identity_and_shared_project_mcp() {
    let (home, data) = common::sandbox();
    agenthub_core::paths::init_overrides(&home, &data);
    let config = AppConfig {
        store_root: data.join("store"),
        custom_agents: vec![custom("trae", "Trae"), custom("trae-cn", "Trae CN")],
        ..AppConfig::default()
    };
    let hub = AgentHub::open(config).unwrap();

    // 1. 身份必须是登记 key，不是展示名：否则 `--agents trae` 永远匹配不上
    let view = hub.settings_view();
    let entry = view
        .agents
        .iter()
        .find(|agent| agent.id == "custom:trae")
        .unwrap_or_else(|| panic!("应存在 custom:trae，实际 {:?}", view.agents));
    assert_eq!(entry.display_name, "Trae", "展示名仍要回 config 里查");
    assert!(
        view.agents.iter().all(|agent| agent.id != "custom:Trae"),
        "不允许再用 display_name 当身份"
    );

    // 2. 两版共用同一个项目级 MCP 文件 → 只写一次，并注明原因
    let project = data.join("proj");
    fs::create_dir_all(&project).unwrap();
    let scope = Scope::Project {
        root: project.clone(),
    };
    let agents = [
        AgentId::Custom("trae".to_string()),
        AgentId::Custom("trae-cn".to_string()),
    ];
    let servers = [demo_server()];

    let plan = hub.plan_mcp_write(&servers, &agents, &scope).unwrap();
    assert_eq!(
        plan.op_count(),
        1,
        "两版共用 .trae/mcp.json，只能产生 1 个写操作，实际 {:#?}",
        plan.ops
    );
    assert!(
        plan.notes.iter().any(|note| note.contains(".trae")),
        "应说明为什么跳过一次写入，实际 {:?}",
        plan.notes
    );

    hub.apply(&plan, &scope).unwrap();
    let written = fs::read_to_string(project.join(".trae/mcp.json")).unwrap();
    assert!(
        written.contains("demo"),
        "写入内容应包含 MCP 条目：{written}"
    );

    // 幂等：已是最新时不再产生操作
    let again = hub.plan_mcp_write(&servers, &agents, &scope).unwrap();
    assert!(again.is_empty(), "重复写入必须幂等");
}
