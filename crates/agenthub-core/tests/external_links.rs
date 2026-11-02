mod common;

use std::fs;

use agenthub_core::model::agent::Scope;

/// 回归：多份「指向受管目录之外」的外部链接，不能被当成基线搬进中央仓库。
#[test]
fn foreign_links_are_never_used_as_baseline() {
    let (home, data) = common::sandbox();
    common::setup(&home);

    // 外部目录：模拟用户已有的 ~/.agents/skills 共享目录
    let external = data.join("external/shared-skill");
    common::write_skill(&external, "shared-skill", "external content");

    for agent in [".claude", ".codex", ".hermes"] {
        fs::create_dir_all(home.join(agent).join("skills")).unwrap();
        let link = home.join(agent).join("skills/shared-skill");
        common::link_dir(&external, &link);
    }

    let hub = common::hub(&home, &data);
    let report = hub.scan(&Scope::User).unwrap();

    // 三份都是外部链接：扫描阶段就应判定「无需处理」，不进入重复组
    let members: Vec<_> = report
        .records
        .iter()
        .filter(|r| r.name == "shared-skill")
        .collect();
    assert_eq!(members.len(), 3);
    assert!(
        members.iter().all(|m| matches!(
            m.link,
            agenthub_core::model::skill::LinkState::Foreign { .. }
        )),
        "三份都应是外部链接"
    );
    assert!(
        !report.duplicates.iter().any(|d| d.name == "shared-skill"),
        "全为外部链接的组不应被算作待处理重复"
    );

    let plan = hub
        .plan_dedupe(Some(&["shared-skill".to_string()]), None)
        .unwrap();
    assert!(
        plan.is_empty(),
        "没有实体副本可作基线时不应生成任何搬迁操作，实际: {:?}",
        plan.ops.iter().map(|o| o.describe()).collect::<Vec<_>>()
    );

    // 外部目录与所有链接保持不变
    assert!(external.join("SKILL.md").is_file());
    for agent in [".claude", ".codex", ".hermes"] {
        let link = home.join(agent).join("skills/shared-skill");
        common::assert_linked(&link);
        assert!(link.join("SKILL.md").is_file());
    }
    assert!(
        !data.join("store/shared-skill").exists(),
        "仓库不应出现条目"
    );
}
