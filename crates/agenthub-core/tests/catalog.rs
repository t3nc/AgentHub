//! 常见 Agent 目录的静态表自洽性，以及「纳入受管 / 移出受管」的落盘行为。
//!
//! 注意：`paths::init_overrides` 是 `OnceLock`，同一测试进程只能钉一次 home/data，
//! 所以只有 `managed_agent_roundtrip` 建沙箱，其余用例都是纯函数断言。

mod common;

use std::collections::HashSet;
use std::path::PathBuf;

use agenthub_core::AppConfig;
use agenthub_core::Scope;
use agenthub_core::ext::catalog::{self, Managed};
use agenthub_core::model::agent::AgentId;
use agenthub_core::model::mcp::{McpFormat, McpServer, Transport};

#[test]
fn catalog_table_is_consistent() {
    let specs = catalog::specs();
    assert_eq!(specs.len(), 16, "目录条目数量变了要同步 UI 分组与文档");

    let mut keys = HashSet::new();
    let mut builtin = 0;
    for spec in specs {
        assert!(keys.insert(spec.key), "重复的 key: {}", spec.key);
        assert!(
            !spec.official.is_empty() || spec.needs_confirm,
            "{} 官方来源缺失",
            spec.key
        );
        // 名字可能是原文也可能是词典 key，取词后都必须非空
        assert!(!spec.name.resolve().is_empty());
        assert!(!spec.vendor.resolve().is_empty());
        assert!(
            !catalog::plan_steps(spec).is_empty(),
            "{} 没有安装步骤",
            spec.key
        );
        if spec.builtin.is_some() {
            builtin += 1;
        }

        // 不能既「不支持受管」又不给原因：UI 靠 blocked 解释置灰按钮。
        // 共用内置配置的（shared_agent）和被内置接管的（builtin）不走这条。
        if catalog::managed_def(spec.key).is_err()
            && spec.builtin.is_none()
            && spec.shared_agent.is_none()
        {
            assert!(
                spec.blocked.is_some(),
                "{} 没有受管模板也没有 blocked 原因",
                spec.key
            );
        }
    }
    assert_eq!(builtin, 6, "内置 agent 应有 6 个（含 OpenCode）");
}

#[test]
fn managed_def_respects_boundaries() {
    // 内置平台已受管，不能再登记一次。
    // 文案随界面语言变化（CI 的 locale 是 en_US），断言变体 + 是否点名了那个平台。
    let err = catalog::managed_def("claude-code").unwrap_err();
    assert!(matches!(err, agenthub_core::Error::Conflict(_)), "{err}");
    assert!(err.to_string().contains("Claude Code"), "{err}");

    // 与内置适配器共用配置的（OpenAI 桌面端 → ~/.codex）明确拒绝并说明原因
    let err = catalog::managed_def("chatgpt-desktop").unwrap_err();
    assert!(matches!(err, agenthub_core::Error::Conflict(_)), "{err}");
    assert!(err.to_string().contains("Codex"), "{err}");

    // 仍然没有本地约定的产品拒绝，并带上原因
    assert!(catalog::managed_def("manus").is_err());
    assert!(catalog::managed_def("pi").is_err());

    // WorkBuddy：实测有 ~/.workbuddy/skills 与 ~/.workbuddy/mcp.json，可以接管
    let def = catalog::managed_def("workbuddy").unwrap();
    assert_eq!(def.id, "workbuddy");
    assert_eq!(def.skills_dir.as_deref(), Some("~/.workbuddy/skills"));
    assert_eq!(def.mcp_file.as_deref(), Some("~/.workbuddy/mcp.json"));
    assert_eq!(def.instructions.len(), 2);
    assert!(matches!(def.mcp_format, Some(McpFormat::Json { .. })));

    // Claude Desktop：只接管 MCP；Store 版路径不存在时回退到 %APPDATA%（不是 None）
    let def = catalog::managed_def("claude-desktop").unwrap();
    assert_eq!(def.id, "claude-desktop");
    let mcp = def.mcp_file.expect("必须有 MCP 文件");
    assert!(mcp.ends_with("claude_desktop_config.json"), "{mcp}");
    assert!(def.skills_dir.is_none());
    assert!(matches!(def.mcp_format, Some(McpFormat::Json { .. })));

    // OpenCode 已升级为内置（技能来自 .agents/skills 与 .claude/skills），不能再登记
    let err = catalog::managed_def("opencode").unwrap_err();
    assert!(matches!(err, agenthub_core::Error::Conflict(_)), "{err}");

    // Trae：项目级 MCP
    let def = catalog::managed_def("trae").unwrap();
    assert_eq!(def.project_mcp_file.as_deref(), Some(".trae/mcp.json"));

    // 未知 key 不能静默成功
    assert!(catalog::managed_def("nope").is_err());
}

/// 国内版 / 国际版是两条独立记录：同族相邻、路径各自独立、可分别登记。
#[test]
fn intl_and_cn_variants_are_separate_entries() {
    let keys: Vec<&str> = catalog::specs().iter().map(|s| s.key).collect();
    let pos = |key: &str| {
        keys.iter()
            .position(|k| *k == key)
            .unwrap_or_else(|| panic!("目录里没有 {key}：{keys:?}"))
    };
    // UI 不做分组排序，靠表内顺序让两版相邻
    assert_eq!(pos("codebuddy") + 1, pos("codebuddy-ide"));
    assert_eq!(pos("codebuddy-ide") + 1, pos("workbuddy"));
    assert_eq!(pos("workbuddy") + 1, pos("workbuddy-ai"));
    assert_eq!(pos("workbuddy-ai") + 1, pos("trae"));
    assert_eq!(pos("trae") + 1, pos("trae-cn"));

    // CodeBuddy IDE：只检测，不接管（MCP/技能位置未核实）
    let ide = catalog::find("codebuddy-ide").expect("要有 CodeBuddy IDE 条目");
    assert!(ide.needs_confirm);
    assert!(ide.blocked.is_some(), "没有模板就必须给 blocked 原因");
    assert!(
        ide.win_config.iter().any(|p| p.contains("CodeBuddy CN")),
        "国内版 IDE 目录 %APPDATA%/CodeBuddy CN 要覆盖：{:?}",
        ide.win_config
    );
    assert!(
        ide.mac_config.iter().any(|p| p.contains("CodeBuddy CN")),
        "macOS 国内版同样是 CodeBuddy CN：{:?}",
        ide.mac_config
    );

    // WorkBuddy 国际版：dataFolderName 换成 .workbuddy-ai，不碰国内版路径
    let def = catalog::managed_def("workbuddy-ai").expect("WorkBuddy AI 应该可登记");
    assert_eq!(def.id, "workbuddy-ai");
    assert_eq!(def.skills_dir.as_deref(), Some("~/.workbuddy-ai/skills"));
    assert_eq!(def.mcp_file.as_deref(), Some("~/.workbuddy-ai/mcp.json"));
    assert!(
        def.instructions.is_empty(),
        "国际版本机未见 SOUL.md / USER.md，不该接管人格文件"
    );
    let cn = catalog::managed_def("workbuddy").expect("WorkBuddy 国内版应该可登记");
    assert_eq!(cn.id, "workbuddy", "老配置里的 id 不能变");
    assert_eq!(cn.skills_dir.as_deref(), Some("~/.workbuddy/skills"));

    // Trae：两版 id 不同，但项目级 MCP 沿用同名文件
    let intl = catalog::managed_def("trae").expect("Trae 国际版应该可登记");
    assert_eq!(intl.id, "trae");
    assert_eq!(intl.project_mcp_file.as_deref(), Some(".trae/mcp.json"));
    let cn = catalog::managed_def("trae-cn").expect("Trae CN 应该可登记");
    assert_eq!(cn.id, "trae-cn");
    assert_eq!(cn.project_mcp_file.as_deref(), Some(".trae/mcp.json"));

    // 检测候选覆盖两版目录
    let trae_cn = catalog::find("trae-cn").unwrap();
    assert!(
        trae_cn
            .win_apps
            .contains(&"%LOCALAPPDATA%/Programs/Trae CN")
    );
    assert!(trae_cn.mac_apps.contains(&"/Applications/Trae CN.app"));
    assert!(trae_cn.win_config.contains(&"%APPDATA%/Trae CN"));
}

/// Store 版配置在多个候选位置时，选第一个存在的；都不存在才回退首选。
#[test]
fn pick_existing_prefers_real_file() {
    let missing = PathBuf::from("/definitely/not/here/agenthub-catalog-test");
    let present = std::env::temp_dir();

    let (idx, exists) = catalog::pick_existing_index(&[missing.clone(), present]);
    assert!(exists, "第二个候选存在时应该命中");
    assert_eq!(idx, 1);

    let (idx, exists) = catalog::pick_existing_index(&[missing.clone(), missing.clone()]);
    assert!(!exists, "都不存在时不上报存在");
    assert_eq!(idx, 0, "都不存在时回退首选");
}

/// Store（MSIX）检测只认包目录，不认传统 Roaming 路径。
#[test]
fn store_packages_are_declared() {
    let claude = catalog::find("claude-desktop").unwrap();
    assert!(
        claude.win_msix.contains(&"Claude_pzs8sxrjxfjjc"),
        "Claude Desktop 要能识别 Store 包"
    );
    assert!(
        claude
            .win_config
            .iter()
            .any(|p| p.contains("Packages") && p.ends_with("claude_desktop_config.json")),
        "要包含 MSIX 虚拟化后的配置路径"
    );

    let openai = catalog::find("chatgpt-desktop").unwrap();
    assert!(openai.win_msix.contains(&"OpenAI.Codex"));
    assert_eq!(
        openai.shared_agent.as_ref().map(|id| id.slug()),
        Some("codex".into())
    );

    // macOS 没有 MSIX 概念
    assert!(
        claude
            .mac_config
            .iter()
            .all(|p| !p.contains("%LOCALAPPDATA%"))
    );
}

#[test]
fn managed_agent_roundtrip() {
    let (home, data) = common::sandbox();
    let mut hub = common::hub(&home, &data);

    let entries = hub.agent_catalog(false);
    assert_eq!(entries.len(), 16);
    // 沙箱里什么都没装：内置条目拿到的 CLI 来自真实 PATH，不在这里断言；
    // 但条目自身必须齐全。
    let claude = entries.iter().find(|e| e.key == "claude-code").unwrap();
    assert_eq!(claude.managed, Managed::Builtin);
    assert!(!claude.steps.is_empty());

    // 与内置适配器共用配置的条目不该显示成「不可受管」
    let openai = entries.iter().find(|e| e.key == "chatgpt-desktop").unwrap();
    assert_eq!(openai.managed, Managed::Shared);
    assert_eq!(openai.shared_agent.as_deref(), Some("codex"));

    // WorkBuddy 实测有 ~/.workbuddy，沙箱里未登记 → 可登记，并列出技能目录
    let workbuddy = entries.iter().find(|e| e.key == "workbuddy").unwrap();
    assert_eq!(workbuddy.managed, Managed::Addable);
    // 断言路径与作用域而不是标签文案：标签随界面语言变化
    assert!(
        workbuddy
            .managed_targets
            .iter()
            .any(|t| t.scope == "user" && t.path.contains("workbuddy/skills")),
        "确认面板要能列出 WorkBuddy 的技能目录，实际 {:?}",
        workbuddy.managed_targets
    );

    // ---- 登记 ----
    let entries = hub.add_managed_agent("claude-desktop").unwrap();
    let entry = entries.iter().find(|e| e.key == "claude-desktop").unwrap();
    assert_eq!(entry.managed, Managed::Managed);
    assert_eq!(entry.managed_id.as_deref(), Some("claude-desktop"));
    assert!(entry.managed_note.is_some());
    assert!(
        entry
            .managed_targets
            .iter()
            .any(|t| t.path.contains("claude_desktop_config.json")),
        "确认前要能列出将写入的 MCP 文件，实际 {:?}",
        entry.managed_targets
    );

    // 配置真的落盘了（重启后仍生效）
    let saved = AppConfig::load_from(&agenthub_core::paths::config_path());
    assert!(
        saved.custom_agents.iter().any(|d| d.id == "claude-desktop"),
        "config.json 里应该有登记"
    );

    // 适配器重建了：用户级 MCP 目标里出现这条自定义平台
    let targets = hub.mcp_files(&Scope::User);
    assert!(
        targets.iter().any(|f| f.agent.slug().starts_with("custom:")
            && f.path.ends_with("claude_desktop_config.json")),
        "登记后应立即参与 MCP 分发"
    );

    // ---- 幂等 ----
    hub.add_managed_agent("claude-desktop").unwrap();
    let saved = AppConfig::load_from(&agenthub_core::paths::config_path());
    assert_eq!(
        saved
            .custom_agents
            .iter()
            .filter(|d| d.id == "claude-desktop")
            .count(),
        1,
        "重复登记不能写出多条"
    );

    // ---- 移出 ----
    let entries = hub.remove_managed_agent("claude-desktop").unwrap();
    let entry = entries.iter().find(|e| e.key == "claude-desktop").unwrap();
    assert_eq!(entry.managed, Managed::Addable);
    let saved = AppConfig::load_from(&agenthub_core::paths::config_path());
    assert!(saved.custom_agents.is_empty(), "移出后配置里不该留登记");

    // ---- WorkBuddy 往返：技能目录 + MCP 文件都要落进配置 ----
    hub.add_managed_agent("workbuddy").unwrap();
    let saved = AppConfig::load_from(&agenthub_core::paths::config_path());
    let def = saved
        .custom_agents
        .iter()
        .find(|d| d.id == "workbuddy")
        .expect("WorkBuddy 应该被登记");
    assert_eq!(def.skills_dir.as_deref(), Some("~/.workbuddy/skills"));
    assert_eq!(def.mcp_file.as_deref(), Some("~/.workbuddy/mcp.json"));
    assert_eq!(def.instructions.len(), 2);
    hub.remove_managed_agent("workbuddy").unwrap();
    let saved = AppConfig::load_from(&agenthub_core::paths::config_path());
    assert!(saved.custom_agents.is_empty(), "移出后不该留登记");

    // ---- Trae 两版同时登记：项目级 MCP 指向同一个 .trae/mcp.json，只能产生一个写操作 ----
    hub.add_managed_agent("trae").unwrap();
    hub.add_managed_agent("trae-cn").unwrap();
    let saved = AppConfig::load_from(&agenthub_core::paths::config_path());
    assert_eq!(
        saved.custom_agents.len(),
        2,
        "两版各自一条 custom_agent，互不覆盖"
    );

    let project = home.join("proj");
    std::fs::create_dir_all(&project).unwrap();
    let servers = vec![McpServer {
        name: "agenthub-test".into(),
        transport: Transport::Stdio {
            command: "npx".into(),
            args: vec!["-y".into(), "test-mcp".into()],
            env: Default::default(),
            cwd: None,
        },
        enabled: true,
        timeout_secs: None,
        note: None,
    }];
    let scope = Scope::Project { root: project };
    // 两版都声明了同一个项目级文件；按实际适配器给出的 agent 取，不猜 slug
    let trae_agents: Vec<AgentId> = hub
        .mcp_files(&scope)
        .into_iter()
        .filter(|f| f.path.ends_with(".trae/mcp.json"))
        .map(|f| f.agent)
        .collect();
    assert_eq!(trae_agents.len(), 2, "两版各声明一次 .trae/mcp.json");
    let plan = hub.plan_mcp_write(&servers, &trae_agents, &scope).unwrap();
    assert_eq!(
        plan.op_count(),
        1,
        "同一个项目级文件不能写两次：{:?}",
        plan.notes
    );
    assert!(
        plan.notes.iter().any(|n| n.contains(".trae")),
        "跳过的那个要在 note 里说明：{:?}",
        plan.notes
    );
    hub.remove_managed_agent("trae").unwrap();
    hub.remove_managed_agent("trae-cn").unwrap();

    // 内置平台不能移出
    assert!(hub.remove_managed_agent("codex").is_err());
}
