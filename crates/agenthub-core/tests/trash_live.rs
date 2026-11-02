//! 系统回收站（macOS 废纸篓 / Windows 回收站）的真机用例。
//!
//! **默认跳过**：只有 `TRASH_LIVE=1` 时才真的把一个目录丢进系统回收站——本机跑一次就会在
//! Finder / 资源管理器里留下记录，不适合放进每次 `cargo test`。所以 `scripts/check-macos.sh`
//! 与 `check-windows.ps1` 默认不设这个变量，需要人工验时再加。
//!
//! 契约（两条路都算通过，但必须区分清楚）：
//! 1. 成功 → 结果日志写「移入回收站 …」，原路径消失，批次回滚能原样恢复；
//! 2. 失败（无 GUI 会话 / Finder AppleEvent 超时 / 授权缺失）→ 日志写「回收站不可用（…），
//!    已改存备份 …」，原路径同样消失，回滚依然能恢复。降级是设计内行为，不是缺陷。
//!
//! 一个测试进程只能钉一次 home/data，所以整个文件只有一个会建沙箱的用例。

mod common;

use std::fs;

use agenthub_core::model::agent::Scope;
use agenthub_core::model::change::{DeleteOptions, OpAction};

fn live() -> bool {
    match std::env::var("TRASH_LIVE").ok().as_deref() {
        Some("1") => true,
        _ => {
            eprintln!("跳过：未设置 TRASH_LIVE=1（会真的往系统回收站丢目录）");
            false
        }
    }
}

/// 走完整链路：去重收敛 → 勾选删除（进回收站）→ 校验日志与回滚。
#[test]
fn delete_skill_goes_to_system_trash_or_stashes() {
    if !live() {
        return;
    }
    let (home, data) = common::sandbox();
    common::setup(&home);
    let hub = common::hub(&home, &data);
    let store_root = data.join("store");

    let dedupe = hub.plan_dedupe(None, None).unwrap();
    hub.apply(&dedupe, &Scope::User).unwrap();
    let body = store_root.join("cloudflare");
    assert!(body.is_dir(), "收敛后仓库应有 cloudflare");

    let plan = hub
        .plan_delete(
            std::slice::from_ref(&body),
            DeleteOptions {
                use_trash: true,
                with_links: true,
            },
        )
        .unwrap();
    assert!(
        plan.ops.iter().any(|op| matches!(
            &op.action,
            OpAction::RemovePath {
                use_trash: true,
                ..
            }
        )),
        "计划里真身这一步必须带 use_trash"
    );

    let result = hub.apply(&plan, &Scope::User).unwrap();
    let joined = result.log.join("\n");
    assert!(!body.exists(), "无论进回收站还是备份，原路径都该消失");

    let trashed = joined.contains("移入回收站") || joined.contains("移入废纸篓");
    let stashed = joined.contains("回收站不可用") && joined.contains("已改存备份");
    assert!(
        trashed || stashed,
        "日志必须明确是进了回收站还是退回备份，实际：\n{joined}"
    );
    if trashed {
        eprintln!("实测：已进入系统回收站（Finder 里应能「放回原处」）");
    } else {
        eprintln!("实测：系统回收站不可用，已按设计降级为批次备份：\n{joined}");
    }

    // 回滚：两条路都必须能恢复
    let undone = hub.rollback(&result.batch_id).unwrap();
    eprintln!("回滚日志：{undone:#?}");
    assert!(body.join("SKILL.md").is_file(), "回滚后真身应恢复");
    for agent in [".claude", ".codex", ".hermes", ".gemini"] {
        let link = home.join(agent).join("skills/cloudflare");
        assert!(
            agenthub_core::platform::is_link_like(&link),
            "{} 回滚后应恢复成链接（当前平台 {}）",
            link.display(),
            std::env::consts::OS
        );
        assert!(
            link.join("SKILL.md").is_file(),
            "{} 回滚后应能读到真身内容",
            link.display()
        );
    }
}

/// 只验 `trash` crate 本身在本机的行为：成功就把临时目录丢进回收站并读回。
///
/// 与 UI 无关，用来回答「这台机器上系统回收站到底可不可用」，结论会写进开发日志。
#[test]
fn trash_crate_self_check() {
    if !live() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let victim = dir.path().join("agenthub-trash-selftest");
    fs::create_dir_all(&victim).unwrap();
    fs::write(victim.join("SKILL.md"), "# selftest\n").unwrap();

    match trash::delete(&victim) {
        Ok(()) => {
            assert!(!victim.exists(), "进回收站后原路径应消失");
            eprintln!("实测：trash::delete 成功（macOS 废纸篓 / Windows 回收站可用）");
        }
        Err(err) => {
            assert!(victim.exists(), "失败时不应动到原目录");
            eprintln!("实测：trash::delete 失败（{err}），apply 会退回批次备份");
        }
    }
}
