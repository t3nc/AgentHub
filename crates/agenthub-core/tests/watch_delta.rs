//! 外部变更快照的差异判定（纯函数，不需要 hub / 全局 override）。

use std::fs;
use std::path::PathBuf;

use agenthub_core::watch;

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "agenthub-watch-{tag}-{}-{}",
        std::process::id(),
        agenthub_core::paths::ts_label()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn added_and_removed_are_reported() {
    let root = scratch("addrm");
    let first = watch::snapshot(&root);
    assert!(first.is_empty(), "空目录不应有条目");

    fs::create_dir_all(root.join("cloudflare")).unwrap();
    let second = watch::snapshot(&root);
    let delta = watch::diff(&first, &second);
    assert_eq!(delta.added.len(), 1, "新目录应记为新增");
    assert!(delta.added[0].ends_with("cloudflare"));
    assert!(delta.removed.is_empty() && delta.changed.is_empty());

    fs::remove_dir_all(root.join("cloudflare")).unwrap();
    let third = watch::snapshot(&root);
    let delta = watch::diff(&second, &third);
    assert_eq!(delta.removed.len(), 1, "删掉目录应记为删除");
    assert!(delta.added.is_empty());

    fs::remove_dir_all(&root).ok();
}

#[test]
fn content_change_is_detected() {
    let root = scratch("content");
    let file = root.join("config.json");
    fs::write(&file, "{\"a\":1}").unwrap();
    let first = watch::snapshot(&root);

    // 大小不同 → 大小或 mtime 至少有一个变
    fs::write(&file, "{\"a\":1,\"b\":2}").unwrap();
    let second = watch::snapshot(&root);
    let delta = watch::diff(&first, &second);
    assert!(
        delta.changed.iter().any(|p| p.ends_with("config.json")),
        "内容变化应被报告，实际 {:?}",
        delta
    );

    fs::remove_dir_all(&root).ok();
}

#[test]
fn unchanged_tree_is_quiet() {
    let root = scratch("quiet");
    fs::write(root.join("SKILL.md"), "# demo\n").unwrap();
    let first = watch::snapshot(&root);
    let second = watch::snapshot(&root);
    assert!(watch::diff(&first, &second).is_empty());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn snapshot_all_merges_roots_and_skips_missing() {
    let a = scratch("multi-a");
    let b = scratch("multi-b");
    fs::write(a.join("SKILL.md"), "# a\n").unwrap();
    fs::write(b.join("SKILL.md"), "# b\n").unwrap();

    let missing = std::env::temp_dir().join("agenthub-watch-not-there");
    let snap = watch::snapshot_all(&[a.clone(), b.clone(), missing]);
    assert_eq!(snap.len(), 2, "两个根各一个文件，缺失根被忽略");
    assert!(snap.keys().any(|p| p.starts_with(&a)));
    assert!(snap.keys().any(|p| p.starts_with(&b)));

    fs::remove_dir_all(&a).ok();
    fs::remove_dir_all(&b).ok();
}

#[test]
fn system_junk_is_ignored() {
    let root = scratch("junk");
    fs::write(root.join("desktop.ini"), "x").unwrap();
    fs::write(root.join("Thumbs.db"), "x").unwrap();
    let snap = watch::snapshot(&root);
    assert!(
        snap.is_empty(),
        "系统垃圾文件不应触发变更，实际 {:?}",
        snap.keys().collect::<Vec<_>>()
    );
    fs::remove_dir_all(&root).ok();
}

/// agent CLI 的运行时文件（会话历史、sqlite 日志库）不是「用户改了东西」。
#[test]
fn runtime_noise_does_not_trigger() {
    let root = scratch("noise");
    let noisy = [
        "history.jsonl",
        "session_index.jsonl",
        "stats-cache.json",
        "daemon.log",
        "daemon.status.json",
        "logs_2.sqlite",
        "logs_2.sqlite-wal",
        "state_5.sqlite-shm",
        "scratch-cc-switch.db",
    ];
    for name in noisy {
        fs::write(root.join(name), "x").unwrap();
    }
    fs::write(root.join("settings.json"), "{}").unwrap();

    let snap = watch::snapshot(&root);
    assert_eq!(
        snap.len(),
        1,
        "只应留下 settings.json，实际 {:?}",
        snap.keys().collect::<Vec<_>>()
    );

    // 运行时文件继续增长，不该产出任何差异
    for name in noisy {
        fs::write(root.join(name), "xxxxxx").unwrap();
    }
    let delta = watch::diff(&snap, &watch::snapshot(&root));
    assert!(
        delta.is_empty(),
        "运行时写入被判成外部变更：{}（变更 {:?}）",
        delta.summary(),
        delta.changed
    );
    fs::remove_dir_all(&root).ok();
}

/// 配置根是白名单：名单外的文件一律不看，名单内的改动照常报。
#[test]
fn whitelist_root_only_tracks_listed_names() {
    let root = scratch("whitelist");
    fs::write(root.join("config.toml"), "model = \"a\"\n").unwrap();
    fs::write(root.join("history.jsonl"), "1\n").unwrap();
    fs::create_dir_all(root.join("skills")).unwrap();

    let spec = watch::WatchRoot::whitelist(
        root.clone(),
        ["config.toml".to_string(), "settings.json".to_string()],
    );
    let first = watch::snapshot_root(&spec);
    assert_eq!(first.len(), 1, "只有 config.toml 在白名单里");

    fs::write(root.join("history.jsonl"), "1\n2\n").unwrap();
    fs::create_dir_all(root.join("projects")).unwrap();
    let delta = watch::diff(&first, &watch::snapshot_root(&spec));
    assert!(
        delta.is_empty(),
        "白名单外的写入不该出事，实际 {}",
        delta.summary()
    );

    fs::write(root.join("config.toml"), "model = \"brand-new\"\n").unwrap();
    let delta = watch::diff(&first, &watch::snapshot_root(&spec));
    assert!(
        delta.changed.iter().any(|p| p.ends_with("config.toml")),
        "白名单内的配置文件改动必须照常上报，实际 {:?}",
        delta
    );

    fs::remove_dir_all(&root).ok();
}

/// 同一个根出现两次（技能目录 + 配置目录重叠）时，「全部关注」优先。
#[test]
fn watch_root_merge_keeps_widest_scope() {
    let mut spec = watch::WatchRoot::whitelist("/tmp/x", ["a.json".to_string()]);
    spec.merge(Some(vec!["b.json".to_string()]));
    assert_eq!(
        spec.only.as_deref(),
        Some(["a.json".to_string(), "b.json".to_string()].as_slice())
    );

    spec.merge(None);
    assert_eq!(spec.only, None, "任一方是全部关注，结果就是全部关注");
    assert!(spec.accepts("anything.md"));
}
