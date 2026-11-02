//! 「agent 独有技能」白名单：computer-use 这类各 agent 实现互不通用的技能
//! 不参与去重接管 / 链接修复 / 重复统计，但技能本身照常可用。

mod common;

use agenthub_core::config::AppConfig;
use agenthub_core::model::agent::Scope;
use agenthub_core::model::change::FileOp;

/// 计划里的操作是否碰过某个技能名（`primary_path` 覆盖 MoveIntoStore 的源目录与各条链接）。
fn touches(op: &FileOp, name: &str) -> bool {
    op.primary_path()
        .file_name()
        .map(|file| file.to_string_lossy() == name)
        .unwrap_or(false)
}

#[test]
fn config_defaults_are_editable_and_not_backfilled() {
    let cfg = AppConfig::default();
    assert!(
        cfg.is_agent_local("computer-use"),
        "出厂名单应含 computer-use"
    );
    assert!(
        cfg.is_agent_local("Computer-Use"),
        "大小写不敏感：Windows 目录名与手改配置都不能漏判"
    );
    assert!(cfg.is_agent_local(" browser-use "), "两端空白要被忽略");
    assert!(!cfg.is_agent_local("cloudflare"));
    assert!(!cfg.is_agent_local(""), "空名字永远不算命中");

    // 老配置没有这个字段时，靠 serde(default = …) 拿到默认名单
    let legacy: AppConfig = serde_json::from_str("{}").unwrap();
    assert!(
        legacy.is_agent_local("computer-use"),
        "没有该字段的老配置应退到默认名单"
    );

    // 用户清空后写回 `[]`，读回来必须还是空 —— 不能被默认值悄悄回填
    let cleared = AppConfig {
        agent_local_skills: Vec::new(),
        ..AppConfig::default()
    };
    let text = serde_json::to_string(&cleared).unwrap();
    let back: AppConfig = serde_json::from_str(&text).unwrap();
    assert!(
        back.agent_local_skills.is_empty(),
        "显式清空的名单不得被默认值回填，实际 {:?}",
        back.agent_local_skills
    );
    assert!(!back.is_agent_local("computer-use"));

    // 标记 / 取消 + 归一化（trim + 小写 + 去重）
    let mut cfg = AppConfig::default();
    cfg.set_agent_local("  My-Skill ", true);
    let count = |cfg: &AppConfig, name: &str| {
        cfg.agent_local_skills
            .iter()
            .filter(|known| known.eq_ignore_ascii_case(name))
            .count()
    };
    assert_eq!(count(&cfg, "my-skill"), 1, "应写入归一化后的小写");
    cfg.set_agent_local("MY-SKILL", true);
    assert_eq!(count(&cfg, "my-skill"), 1, "重复标记不得叠条目");
    assert!(cfg.is_agent_local("my-skill"));
    cfg.set_agent_local("my-skill", false);
    assert!(!cfg.is_agent_local("my-skill"));

    // 默认名单里的名字同样可以逐条取消
    cfg.set_agent_local("computer-use", false);
    assert!(!cfg.is_agent_local("computer-use"));
}

#[test]
fn agent_local_skills_skip_dedupe_repair_and_stats() {
    let (home, data) = common::sandbox();
    common::setup(&home);
    // 两个 agent 各一份 computer-use，内容不同（各家实现本来就不通用）
    common::write_skill(
        &home.join(".claude/skills/computer-use"),
        "computer-use",
        "claude impl",
    );
    common::write_skill(
        &home.join(".codex/skills/computer-use"),
        "computer-use",
        "codex impl",
    );
    let mut hub = common::hub(&home, &data);

    let report = hub.scan(&Scope::User).unwrap();
    let local: Vec<_> = report
        .records
        .iter()
        .filter(|rec| rec.name == "computer-use")
        .collect();
    assert_eq!(local.len(), 2, "两个 agent 目录各一条记录");
    assert!(
        local.iter().all(|rec| rec.agent_local),
        "默认名单应把 computer-use 标记为 agent 独有"
    );
    assert!(
        report
            .records
            .iter()
            .filter(|rec| rec.name == "cloudflare")
            .all(|rec| !rec.agent_local),
        "未命中名单的技能不得被误标"
    );

    assert!(
        !report
            .duplicates
            .iter()
            .any(|group| group.name == "computer-use"),
        "agent 独有技能不进重复组（浪费统计与体检项都由此派生）"
    );
    assert!(
        report
            .duplicates
            .iter()
            .any(|group| group.name == "cloudflare"),
        "普通同名副本仍应被识别"
    );

    // 一键收敛：不得为它生成搬运 / 建链操作
    let plan = hub.plan_dedupe(None, None).unwrap();
    assert!(
        !plan.ops.iter().any(|op| touches(op, "computer-use")),
        "去重接管不得动 agent 独有技能，实际 {:?}",
        plan.ops.iter().map(|op| op.describe()).collect::<Vec<_>>()
    );
    assert!(
        plan.ops.iter().any(|op| touches(op, "cloudflare")),
        "其它技能仍应正常生成计划"
    );

    // 用户显式点名要收敛它：不生成操作，但必须解释原因
    let named = hub
        .plan_dedupe(Some(&["computer-use".to_string()]), None)
        .unwrap();
    assert_eq!(named.op_count(), 0, "显式点名也不得生成操作");
    assert!(
        named.notes.iter().any(|note| note.contains("computer-use")),
        "显式点名应给出跳过原因，实际 {:?}",
        named.notes
    );

    // 修复链接：不重指向仓库，并汇总一条说明
    let repair = hub.plan_repair().unwrap();
    assert!(
        !repair.ops.iter().any(|op| touches(op, "computer-use")),
        "修复计划不得把 agent 独有技能指回仓库"
    );
    assert!(
        repair
            .notes
            .iter()
            .any(|note| note.contains("computer-use")),
        "修复计划应汇总跳过说明，实际 {:?}",
        repair.notes
    );

    // 取消标记后回到统一管理（配置落盘后重新扫描即生效）
    hub.set_agent_local_skill("computer-use", false).unwrap();
    let after = hub.scan(&Scope::User).unwrap();
    assert!(
        after
            .records
            .iter()
            .filter(|rec| rec.name == "computer-use")
            .all(|rec| !rec.agent_local),
        "取消标记后不应再带 agent_local 标记"
    );
    assert!(
        after
            .duplicates
            .iter()
            .any(|group| group.name == "computer-use"),
        "取消标记后应重新进入重复组"
    );
}
