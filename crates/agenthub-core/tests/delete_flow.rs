mod common;

use std::path::Path;

use agenthub_core::model::agent::Scope;
use agenthub_core::model::change::{DeleteOptions, OpAction};

fn removed_paths(ops: &[agenthub_core::model::change::FileOp]) -> Vec<String> {
    ops.iter()
        .filter_map(|op| match &op.action {
            OpAction::RemovePath { at, .. } => Some(agenthub_core::paths::path_key(at)),
            _ => None,
        })
        .collect()
}

fn key(path: &Path) -> String {
    agenthub_core::paths::path_key(path)
}

/// 一次跑完删除的四种语义：连带链接、只读拒绝、未知路径、保留链接。
#[test]
fn delete_skills_with_links_and_rollback() {
    let (home, data) = common::sandbox();
    common::setup(&home);
    let hub = common::hub(&home, &data);
    let store_root = data.join("store");

    // 先收敛，得到「仓库真身 + 4 条链接」的状态
    let dedupe = hub.plan_dedupe(None, None).unwrap();
    hub.apply(&dedupe, &Scope::User).unwrap();
    let body = store_root.join("cloudflare");
    assert!(body.is_dir(), "收敛后仓库应有 cloudflare");

    let agent_links = [".claude", ".codex", ".hermes", ".gemini"]
        .map(|agent| home.join(agent).join("skills/cloudflare"));
    for link in &agent_links {
        common::assert_linked(link);
    }

    // ── 1) 计划形状：删真身要连带 4 条链接，且链接排在真身之前 ────────────
    let plan = hub
        .plan_delete(std::slice::from_ref(&body), DeleteOptions::safe())
        .unwrap();
    assert_eq!(plan.op_count(), 5, "4 条链接 + 1 个真身");
    let paths = removed_paths(&plan.ops);
    assert_eq!(paths.last().unwrap(), &key(&body), "真身必须最后删");
    for link in &agent_links {
        assert!(
            paths.contains(&key(link)),
            "{} 应被连带删除",
            link.display()
        );
    }
    // 断言结构而不是文案：汇总一条 + 连带清理一条（文案随界面语言变化）
    assert_eq!(
        plan.notes.len(),
        2,
        "计划说明应包含汇总与连带清理两条，实际 {:?}",
        plan.notes
    );

    // ── 2) 保留链接的选项要给出断链风险，且只删 1 项 ────────────────────
    let keep_links = hub
        .plan_delete(
            std::slice::from_ref(&body),
            DeleteOptions {
                use_trash: false,
                with_links: false,
            },
        )
        .unwrap();
    assert_eq!(keep_links.op_count(), 1);
    // 两条风险：永久删除（不进回收站）+ 保留链接会变失效（文案随界面语言变化）
    assert_eq!(
        keep_links.risks.len(),
        2,
        "保留链接时应给出永久删除与断链两条风险，实际 {:?}",
        keep_links.risks
    );

    // ── 3) 真正执行（不进回收站，便于验证回滚） ────────────────────────
    let plan = hub
        .plan_delete(
            std::slice::from_ref(&body),
            DeleteOptions {
                use_trash: false,
                with_links: true,
            },
        )
        .unwrap();
    let result = hub.apply(&plan, &Scope::User).unwrap();
    assert!(!body.exists(), "仓库真身应被删除");
    for link in &agent_links {
        assert!(
            std::fs::symlink_metadata(link).is_err(),
            "{} 应被连带删除",
            link.display()
        );
    }

    let after = hub.scan(&Scope::User).unwrap();
    assert!(
        after.records.iter().all(|r| r.name != "cloudflare"),
        "删除后不应再扫到 cloudflare"
    );

    // ── 4) 回滚：真身与链接都要回来 ────────────────────────────────────
    hub.rollback(&result.batch_id).unwrap();
    assert!(body.join("SKILL.md").is_file(), "回滚后真身应恢复");
    for link in &agent_links {
        assert!(
            link.join("SKILL.md").is_file(),
            "{} 应恢复为链接",
            link.display()
        );
    }

    // ── 5) 只读内置技能：一律跳过 ──────────────────────────────────────
    let readonly = home.join(".codex/skills/.system/imagegen");
    let plan = hub
        .plan_delete(std::slice::from_ref(&readonly), DeleteOptions::safe())
        .unwrap();
    assert!(plan.ops.is_empty(), "只读技能不应产生任何删除操作");
    assert!(
        plan.notes.iter().any(|n| n.contains("imagegen")),
        "跳过说明应点出具体技能，实际 {:?}",
        plan.notes
    );
    assert!(readonly.exists());

    // ── 6) 不在扫描结果里的路径：拒绝执行 ─────────────────────────────
    let stranger = home.join(".claude/skills/never-scanned");
    let plan = hub.plan_delete(&[stranger], DeleteOptions::safe()).unwrap();
    assert!(plan.ops.is_empty());
    assert!(
        plan.notes.iter().any(|n| n.contains("never-scanned")),
        "跳过说明应点出未被扫描到的路径，实际 {:?}",
        plan.notes
    );

    // ── 7) 单独删一个 agent 副本 ─────────────────────────────────────
    let sole = home.join(".claude/skills/only-claude");
    let plan = hub
        .plan_delete(
            std::slice::from_ref(&sole),
            DeleteOptions {
                use_trash: false,
                with_links: true,
            },
        )
        .unwrap();
    assert_eq!(plan.op_count(), 1, "独有技能只产生一条删除");
    hub.apply(&plan, &Scope::User).unwrap();
    assert!(!sole.exists());

    // ── 8) 空选择直接报错，不生成空计划 ───────────────────────────────
    assert!(hub.plan_delete(&[], DeleteOptions::safe()).is_err());
}
