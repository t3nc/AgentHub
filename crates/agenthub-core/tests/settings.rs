//! 设置页的后端契约：校验、保存生效、清空覆盖回退自动检测。
//!
//! 只有 `override_path_takes_effect_after_save` 建沙箱（`paths::init_overrides` 是
//! `OnceLock`，同一测试进程只能钉一次 home/data）。

mod common;

use std::collections::BTreeMap;

use agenthub_core::AppConfig;
use agenthub_core::Scope;
use agenthub_core::api::settings::{self, SettingsInput};
use agenthub_core::config::{AgentPathOverride, CustomAgentDef};
use agenthub_core::model::agent::AgentId;
use agenthub_core::scan::ScanReport;

fn input() -> SettingsInput {
    SettingsInput {
        store_root: "~/.agenthub/store".into(),
        default_strategy: None,
        backups_keep: 10,
        excluded_paths: Vec::new(),
        use_trash_for_dedupe: false,
        backup_config_files: true,
        agent_paths: BTreeMap::new(),
        custom_agents: Vec::new(),
        market_source: String::new(),
        market_registry: String::new(),
    }
}

fn custom(id: &str) -> CustomAgentDef {
    CustomAgentDef {
        id: id.to_string(),
        display_name: id.to_string(),
        skills_dir: Some(format!("~/{id}/skills")),
        project_skills_dir: None,
        mcp_file: None,
        mcp_format: None,
        project_mcp_file: None,
        instructions: Vec::new(),
    }
}

fn has(report: &ScanReport, name: &str) -> bool {
    report
        .records
        .iter()
        .any(|record| record.name == name && record.agent == Some(AgentId::Hermes))
}

#[test]
fn settings_reject_invalid_input() {
    let cfg = AppConfig::default();

    let mut bad = input();
    bad.backups_keep = 0;
    assert!(
        settings::build_config(&cfg, &bad).is_err(),
        "0 份备份应被拒"
    );

    let mut bad = input();
    bad.backups_keep = 101;
    assert!(
        settings::build_config(&cfg, &bad).is_err(),
        "超过 100 份应被拒"
    );

    let mut bad = input();
    bad.store_root = "   ".into();
    assert!(
        settings::build_config(&cfg, &bad).is_err(),
        "空仓库目录应被拒"
    );

    let mut bad = input();
    bad.agent_paths
        .insert("nope".into(), AgentPathOverride::default());
    // 这几条文案都随界面语言变化（CI 的 locale 是 en_US），断言「文案里点名了哪个值」
    // 而不是某个中文词：每条模板都会把出错的那个 id / agent 名带出来。
    let err = settings::build_config(&cfg, &bad).unwrap_err();
    assert!(err.to_string().contains("nope"), "{err}");

    let mut bad = input();
    bad.custom_agents = vec![custom("hermes")];
    let err = settings::build_config(&cfg, &bad).unwrap_err();
    assert!(err.to_string().contains("hermes"), "{err}");

    let mut bad = input();
    bad.custom_agents = vec![custom("dup"), custom("dup")];
    let err = settings::build_config(&cfg, &bad).unwrap_err();
    assert!(err.to_string().contains("dup"), "{err}");

    let mut bad = input();
    let mut empty = custom("no-target");
    empty.skills_dir = None;
    bad.custom_agents = vec![empty];
    let err = settings::build_config(&cfg, &bad).unwrap_err();
    assert!(err.to_string().contains("no-target"), "{err}");

    // 空白覆盖字段会被归一化掉，不会留下一条空覆盖
    let mut ok = input();
    ok.agent_paths.insert(
        "hermes".into(),
        AgentPathOverride {
            skills_dir: Some("   ".into()),
            ..Default::default()
        },
    );
    let cfg2 = settings::build_config(&cfg, &ok).unwrap();
    assert!(cfg2.agent_paths.is_empty(), "空白字段不该落成覆盖");

    // 市场数据源：非法值拒绝，合法值落盘
    let mut bad = input();
    bad.market_source = "nope".into();
    let err = settings::build_config(&cfg, &bad).unwrap_err();
    assert!(err.to_string().contains("nope"), "{err}");

    let mut switched = input();
    switched.market_source = "clawhub".into();
    let cfg3 = settings::build_config(&cfg, &switched).unwrap();
    assert_eq!(cfg3.market.source.as_deref(), Some("clawhub"));
    assert_eq!(
        agenthub_core::market::target(&cfg3).unwrap().source,
        agenthub_core::config::MarketSource::ClawHub
    );
}

#[test]
fn override_path_takes_effect_after_save() {
    let (home, data) = common::sandbox();

    // 先造目录再 open：候选根（`~/.hermes` vs `%LOCALAPPDATA%\hermes`）在构造适配器时
    // 解析一次，目录不存在就会被判成「空」而落到另一个候选上。
    common::write_skill(
        &home.join(".hermes/skills/default-skill"),
        "default-skill",
        "default",
    );
    common::write_skill(
        &home.join("custom-skills/override-skill"),
        "override-skill",
        "custom",
    );

    let mut hub = common::hub(&home, &data);

    let report = hub.scan(&Scope::User).unwrap();
    assert!(has(&report, "default-skill"), "默认目录应被扫到");
    assert!(
        !has(&report, "override-skill"),
        "未覆盖前不该扫到自定义目录"
    );

    // 保存覆盖
    let mut next = input();
    next.store_root = data.join("store").display().to_string();
    next.agent_paths.insert(
        "hermes".into(),
        AgentPathOverride {
            skills_dir: Some(home.join("custom-skills").display().to_string()),
            ..Default::default()
        },
    );
    let view = hub.save_settings(next).unwrap();
    let hermes = view.agents.iter().find(|a| a.id == "hermes").unwrap();
    assert!(hermes.overrides.skills_dir.is_some(), "覆盖应回显到视图");
    assert!(
        hermes
            .effective
            .skills_dir
            .as_deref()
            .unwrap_or_default()
            .ends_with("custom-skills"),
        "生效路径应切到覆盖值：{:?}",
        hermes.effective.skills_dir
    );

    // 重建适配器后立刻生效（不重建就还是旧路径）
    let report = hub.scan(&Scope::User).unwrap();
    assert!(has(&report, "override-skill"), "覆盖后应扫到新目录");
    assert!(!has(&report, "default-skill"), "覆盖后不该再扫默认目录");

    // 配置真的落盘（重启后仍生效）
    let saved = AppConfig::load_from(&agenthub_core::paths::config_path());
    assert!(
        saved.agent_paths.contains_key("hermes"),
        "config.json 里应有覆盖"
    );

    // 清空覆盖 → 回到自动检测
    let mut back = input();
    back.store_root = data.join("store").display().to_string();
    let view = hub.save_settings(back).unwrap();
    let hermes = view.agents.iter().find(|a| a.id == "hermes").unwrap();
    assert!(hermes.overrides.is_empty());
    assert!(
        hermes
            .effective
            .skills_dir
            .as_deref()
            .unwrap_or_default()
            .ends_with(".hermes/skills"),
        "清空后应回到默认目录：{:?}",
        hermes.effective.skills_dir
    );

    let saved = AppConfig::load_from(&agenthub_core::paths::config_path());
    assert!(saved.agent_paths.is_empty(), "清空覆盖要落盘");
}
