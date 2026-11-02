//! 两层技能结构（`skills/<分类>/<技能>/SKILL.md`）的扫描行为。
//!
//! Hermes 的技能就是这么放的（81 个分类技能 + 9 个一层技能）。这里同时钉住三条边界：
//! 一层技能照旧、分类目录展开、隐藏目录（锁 / 缓存）不再被当成技能。

mod common;

use std::collections::BTreeMap;
use std::fs;

use agenthub_core::Scope;
use agenthub_core::api::settings::SettingsInput;
use agenthub_core::config::AgentPathOverride;
use agenthub_core::model::agent::AgentId;

fn settings_input(store: &std::path::Path) -> SettingsInput {
    SettingsInput {
        store_root: store.display().to_string(),
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

fn slash(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[test]
fn scan_expands_categories_and_keeps_readonly() {
    let (home, data) = common::sandbox();
    let mut hub = common::hub(&home, &data);

    let skills = home.join(".claude/skills");

    // 一层：直接有 SKILL.md
    common::write_skill(&skills.join("direct-skill"), "direct-skill", "flat");

    // 两层：分类 / 技能 / SKILL.md
    common::write_skill(&skills.join("category/nested-a"), "nested-a", "nested");
    common::write_skill(&skills.join("category/nested-b"), "nested-b", "nested");
    // 分类里混了一个没有 SKILL.md 的目录：不该单独成为技能
    fs::create_dir_all(skills.join("category/not-a-skill")).unwrap();
    fs::write(skills.join("category/not-a-skill/readme.txt"), "x").unwrap();

    // 老的无 SKILL.md 目录：保持收录，不因这次改动而消失
    fs::create_dir_all(skills.join("legacy-no-md")).unwrap();
    fs::write(skills.join("legacy-no-md/notes.txt"), "x").unwrap();

    // 隐藏锁目录：既没有 SKILL.md 也没有技能子目录 → 跳过
    fs::create_dir_all(skills.join(".locks")).unwrap();
    fs::write(skills.join(".locks/state"), "x").unwrap();

    // 隐藏系统目录：展开，且每条继承只读
    common::write_skill(&skills.join(".system/imagegen"), "imagegen", "sys");

    let report = hub.scan(&Scope::User).unwrap();
    let mut names: Vec<String> = report
        .records
        .iter()
        .filter(|r| r.agent == Some(AgentId::Claude))
        .map(|r| r.name.clone())
        .collect();
    names.sort();

    assert_eq!(
        names,
        vec![
            "direct-skill".to_string(),
            "imagegen".to_string(),
            "legacy-no-md".to_string(),
            "nested-a".to_string(),
            "nested-b".to_string(),
        ],
        "分类目录应被展开，隐藏锁目录应被跳过，无 SKILL.md 的老目录仍保留"
    );
    assert!(
        !names.contains(&"category".to_string()),
        "分类目录本身不该算一条技能"
    );
    assert!(!names.contains(&"not-a-skill".to_string()));

    let nested = report
        .records
        .iter()
        .find(|r| r.name == "nested-a")
        .expect("分类下的技能应被扫出来");
    assert!(
        slash(&nested.path).ends_with("category/nested-a"),
        "路径应指向分类下的技能目录，实际 {}",
        slash(&nested.path)
    );

    let sys = report
        .records
        .iter()
        .find(|r| r.name == "imagegen")
        .unwrap();
    assert!(sys.readonly, ".system 展开出来的技能必须只读");

    let direct = report
        .records
        .iter()
        .find(|r| r.name == "direct-skill")
        .unwrap();
    assert!(!direct.readonly, "普通技能不该被标成只读");

    // 同一目录被两个 agent 声明时只归属第一个（OpenCode 会兼容读 .claude/skills）。
    // 不做去重就会把同一批文件重复计数，还会凭空造出「自己和自己重复」的组。
    let mut next = settings_input(&data.join("store"));
    next.agent_paths.insert(
        "codex".to_string(),
        AgentPathOverride {
            skills_dir: Some(skills.display().to_string()),
            ..Default::default()
        },
    );
    hub.save_settings(next).unwrap();

    let report = hub.scan(&Scope::User).unwrap();
    let claude_count = report
        .records
        .iter()
        .filter(|r| r.agent == Some(AgentId::Claude))
        .count();
    let codex_count = report
        .records
        .iter()
        .filter(|r| r.agent == Some(AgentId::Codex))
        .count();
    assert_eq!(claude_count, 5, "目录仍归先声明的 Claude");
    assert_eq!(codex_count, 0, "重叠目录不该被重复计数");
}
