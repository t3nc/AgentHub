mod common;

use std::fs;
use std::path::Path;

use agenthub_core::model::agent::Scope;
use agenthub_core::model::change::{FileOp, OpAction};

/// 从计划里取出唯一的 MoveIntoStore 源路径。
fn moved_source(ops: &[FileOp]) -> Option<&Path> {
    ops.iter().find_map(|op| match &op.action {
        OpAction::MoveIntoStore { from, .. } => Some(from.as_path()),
        _ => None,
    })
}

fn same(a: &Path, b: &Path) -> bool {
    agenthub_core::paths::paths_eq(a, b)
}

#[test]
fn dedupe_then_rollback() {
    let (home, data) = common::sandbox();
    common::setup(&home);
    let hub = common::hub(&home, &data);

    let report = hub.scan(&Scope::User).unwrap();
    assert!(
        report.records.iter().any(|r| r.name == "cloudflare"),
        "应扫描到 cloudflare"
    );
    let group = report
        .duplicates
        .iter()
        .find(|d| d.name == "cloudflare")
        .expect("应识别出 cloudflare 重复组");
    assert_eq!(group.members.len(), 4);
    assert!(group.identical, "四份内容一致");
    assert!(group.wasted_bytes > 0);
    assert!(
        !report.duplicates.iter().any(|d| d.name == "imagegen"),
        "只读系统项不进入重复组"
    );

    let plan = hub.plan_dedupe(None, None).unwrap();
    assert!(plan.op_count() >= 5, "应有移动 + 4 条链接操作");

    let result = hub.apply(&plan, &Scope::User).unwrap();
    assert!(result.backup_dir.is_some());

    let store_skill = data.join("store/cloudflare");
    assert!(store_skill.is_dir(), "中央仓库应存在 cloudflare");

    // 断言语义而不是实现：Windows 默认策略是目录联接，`is_symlink()` 会是 false
    let store_root = data.join("store");
    for agent in [".claude", ".codex", ".hermes", ".gemini"] {
        let path = home.join(agent).join("skills/cloudflare");
        common::assert_linked(&path);
        let state = agenthub_core::link::link_state(&store_root, &path);
        assert!(
            state.is_linked(),
            "{} 应是受管链接，实际被判为 {}",
            path.display(),
            state.label()
        );
        assert!(
            state
                .target()
                .map(|t| agenthub_core::paths::paths_eq(t, &store_skill))
                .unwrap_or(false),
            "{} 应指向中央仓库，实际 {:?}",
            path.display(),
            state.target()
        );
        assert!(path.join("SKILL.md").is_file(), "链接可正常读取内容");
    }
    assert!(
        store_skill.join("SKILL.md").is_file(),
        "建链接不得动到中央仓库里的真身"
    );

    assert!(
        fs::symlink_metadata(home.join(".codex/skills/.system/imagegen"))
            .unwrap()
            .file_type()
            .is_dir(),
        "只读系统项不得被改写"
    );
    assert!(
        fs::symlink_metadata(home.join(".claude/skills/only-claude"))
            .unwrap()
            .file_type()
            .is_dir(),
        "独有技能不得被改写"
    );

    let again = hub.scan(&Scope::User).unwrap();
    assert!(
        again.duplicates.iter().all(|d| d.name != "cloudflare"),
        "接管后不应再有 cloudflare 重复组"
    );
    assert!(again.stats.shared_skills >= 1, "应统计为跨 agent 共享");

    hub.rollback(&result.batch_id).unwrap();
    for agent in [".claude", ".codex", ".hermes", ".gemini"] {
        let path = home.join(agent).join("skills/cloudflare");
        assert!(
            fs::symlink_metadata(&path).unwrap().file_type().is_dir(),
            "回滚后 {} 应恢复为真实目录",
            path.display()
        );
        assert!(path.join("SKILL.md").is_file());
    }
    assert!(!store_skill.exists(), "回滚后仓库应清空");

    // ── 基线的指定与回退 ───────────────────────────────────────────────
    // 这一段与上面的流程共用沙箱：`paths::init_overrides` 是 OnceLock，
    // 一个测试进程只能钉死一次 home/data，所以拆不出第二个 #[test]。
    let small = home.join(".claude/skills/cloudflare");
    let big = home.join(".hermes/skills/cloudflare");
    // 让 hermes 副本变大且内容不同：默认策略会选它，被指定时必须让位
    fs::write(big.join("extra.md"), "x".repeat(8192)).unwrap();

    let default_plan = hub
        .plan_dedupe(Some(&["cloudflare".to_string()]), None)
        .unwrap();
    let default_src = moved_source(&default_plan.ops).expect("应有搬迁操作");
    assert!(
        same(default_src, &big),
        "默认应选体积最大的实体副本，实际 {}",
        default_src.display()
    );

    let plan = hub
        .plan_dedupe(Some(&["cloudflare".to_string()]), Some(&small))
        .unwrap();
    let src = moved_source(&plan.ops).expect("应有搬迁操作");
    assert!(
        same(src, &small),
        "指定基线后应以 .claude 副本为真身，实际 {}",
        src.display()
    );
    // 断言结构而不是文案：说明里必须点出用户指定的那条路径（文案随界面语言变化）。
    // 走 normalize_lexical 是为了让分隔符与扫描结果一致（Windows 上 join 会留下 `/`）。
    let baseline_note =
        agenthub_core::paths::path_str(&agenthub_core::paths::normalize_lexical(&small));
    assert!(
        plan.notes.iter().any(|n| n.contains(&baseline_note)),
        "计划说明里应点出用户指定的基线路径 {baseline_note}，实际 {:?}",
        plan.notes
    );

    // 不在本组的路径当作没指定：回落到默认基线
    let outsider = home.join(".claude/skills/only-claude");
    let fallback = hub
        .plan_dedupe(Some(&["cloudflare".to_string()]), Some(&outsider))
        .unwrap();
    let fallback_src = moved_source(&fallback.ops).expect("应有搬迁操作");
    assert!(
        same(fallback_src, &big),
        "非本组路径应回落到默认基线，实际 {}",
        fallback_src.display()
    );

    // 链接不能当基线（搬进仓库会变成坏链）：回退 + 说明
    let gemini = home.join(".gemini/skills/cloudflare");
    fs::remove_dir_all(&gemini).unwrap();
    common::link_dir(&small, &gemini);
    let linked = hub
        .plan_dedupe(Some(&["cloudflare".to_string()]), Some(&gemini))
        .unwrap();
    let rejected =
        agenthub_core::paths::path_str(&agenthub_core::paths::normalize_lexical(&gemini));
    assert!(
        linked.notes.iter().any(|n| n.contains(&rejected)),
        "链接作为基线时应回退并点出被拒绝的路径，实际 {:?}",
        linked.notes
    );
    let linked_src = moved_source(&linked.ops).expect("应有搬迁操作");
    assert!(
        same(linked_src, &big),
        "回退后应选体积最大的实体副本，实际 {}",
        linked_src.display()
    );
}
