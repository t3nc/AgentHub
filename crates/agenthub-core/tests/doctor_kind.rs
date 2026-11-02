//! 体检项必须带分类码：顶栏「N 处待处理」靠它决定给哪个快捷操作。
//!
//! 早先只有 `level` + 中文 message，前端想给动作只能 `message.includes("副本")`，
//! 文案一改按钮就失灵。这里钉住后端契约。

mod common;

use agenthub_core::Scope;

#[test]
fn doctor_issues_carry_kind_and_subject() {
    let (home, data) = common::sandbox();
    common::setup(&home);
    let hub = common::hub(&home, &data);

    let report = hub.scan(&Scope::User).expect("扫描应成功");
    let issues = hub.doctor_from(&report);
    assert!(!issues.is_empty(), "沙箱里应有体检项");

    assert!(
        issues.iter().all(|issue| !issue.kind.is_empty()),
        "每条体检项都要带分类码，实际 {:?}",
        issues
            .iter()
            .map(|issue| (&issue.message, &issue.kind))
            .collect::<Vec<_>>()
    );

    let copy = issues
        .iter()
        .find(|issue| issue.kind == "duplicate_copy")
        .expect("4 个 agent 有同名副本，应产出 duplicate_copy");
    assert_eq!(
        copy.subject.as_deref(),
        Some("cloudflare"),
        "收敛副本要能直接拿到技能名"
    );
    assert_eq!(copy.level, "warn");

    // 重复组的汇总项是 info：不能被顶栏当成「待处理」，但仍要有分类码
    assert!(
        issues.iter().any(|issue| issue.kind == "duplicate_group"),
        "重复组应产出 duplicate_group 汇总项"
    );

    // 中央仓库存在时不该报 store_missing
    assert!(
        !issues.iter().any(|issue| issue.kind == "store_missing"),
        "沙箱里中央仓库已建好"
    );
}
