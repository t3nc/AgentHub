//! 外部变更监听的覆盖范围，以及「扫描自己不算外部变更」。
//!
//! 回归的三件事：
//! 1. Provider 改的是 agent 配置目录里的 `settings.json` / `config.toml`，不在 `skills/` 下，
//!    所以配置目录必须在监听列表里（早期版本漏了）。
//! 2. 配置目录**不能整目录监听**：`~/.claude` / `~/.codex` 里混着 `history.jsonl`、
//!    `logs_2.sqlite-wal` 这类 CLI 运行时文件，常驻 agent 每写一次就报一次「外部有变更」，
//!    用户点重扫过几秒又被打回来。配置根必须是文件名白名单。
//! 3. 数据根（`index.db` + 批次）**不能**在监听列表里：扫描会写索引库，监听自己写盘的结果是
//!    「点重扫 → 扫描写库 → 又收到外部变更 → 提示条永远消不掉」，真机上踩过。
//!
//! `paths::init_overrides` 是 `OnceLock`，所以本文件只放一个建沙箱的用例（多场景在用例内顺序做）。

mod common;

use agenthub_core::{Scope, paths, watch};

#[test]
fn watch_targets_cover_agents_but_not_own_data_root() {
    let (home, data) = common::sandbox();
    common::setup(&home);
    let hub = common::hub(&home, &data);

    let targets = hub.watch_targets();
    let keys: Vec<String> = targets
        .iter()
        .map(|root| paths::path_key(&root.path))
        .collect();
    let key_of = |p: &std::path::Path| paths::path_key(p);

    let expect = |p: &std::path::Path, why: &str| {
        assert!(
            keys.contains(&key_of(p)),
            "{why}：{} 不在监听列表里（实际：{keys:?}）",
            p.display()
        );
    };
    let root_of = |p: &std::path::Path| {
        let key = key_of(p);
        targets
            .iter()
            .find(|root| paths::path_key(&root.path) == key)
            .unwrap_or_else(|| panic!("{} 不在监听列表里", p.display()))
    };

    expect(&home.join(".claude/skills"), "技能目录必须被监听");
    expect(&data.join("store"), "中央仓库必须被监听");
    // Provider 写的是 agent 配置目录里的文件，不是 skills 目录
    expect(
        &home.join(".claude"),
        "Claude 配置目录必须被监听（settings.json）",
    );
    expect(
        &home.join(".codex"),
        "Codex 配置目录必须被监听（config.toml）",
    );

    assert!(
        !keys.contains(&key_of(&data)),
        "数据根不能进监听列表：扫描会写 index.db，监听到自己写盘会让提示条永远消不掉（实际：{keys:?}）"
    );

    // 技能根 / 仓库根看全部一级子项
    assert_eq!(
        root_of(&home.join(".claude/skills")).only,
        None,
        "技能目录要关注全部一级子项"
    );
    assert_eq!(
        root_of(&data.join("store")).only,
        None,
        "中央仓库要关注全部"
    );

    // 配置根必须是白名单，且名单里是配置文件而不是运行时文件
    let claude = root_of(&home.join(".claude"));
    assert!(
        claude.accepts("settings.json"),
        "Claude 的 settings.json 在监听范围内"
    );
    assert!(
        claude.accepts(".claude.json"),
        "MCP 配置文件（mcp_files 给出）也要在监听范围内，实际 {:?}",
        claude.only
    );
    assert!(
        !claude.accepts("history.jsonl"),
        "CLI 会话历史不是外部变更，实际白名单 {:?}",
        claude.only
    );
    assert!(
        !claude.accepts("stats-cache.json"),
        "统计缓存不是外部变更，实际白名单 {:?}",
        claude.only
    );

    let codex = root_of(&home.join(".codex"));
    assert!(
        codex.accepts("config.toml"),
        "Codex 的 config.toml 必须被监听"
    );
    assert!(
        !codex.accepts("logs_2.sqlite-wal"),
        "sqlite 日志库不是外部变更，实际白名单 {:?}",
        codex.only
    );
    assert!(
        !codex.accepts("session_index.jsonl"),
        "会话索引不是外部变更，实际白名单 {:?}",
        codex.only
    );

    // 真机场景复演：CLI 持续写运行时文件，快照不能出现任何差异
    for (dir, name) in [
        (home.join(".claude"), "history.jsonl"),
        (home.join(".claude"), "stats-cache.json"),
        (home.join(".codex"), "logs_2.sqlite"),
        (home.join(".codex"), "logs_2.sqlite-wal"),
        (home.join(".codex"), "session_index.jsonl"),
    ] {
        std::fs::create_dir_all(&dir).ok();
        std::fs::write(dir.join(name), "1\n").unwrap();
    }
    let before_noise = watch::snapshot_roots(&targets);
    for (dir, name) in [
        (home.join(".claude"), "history.jsonl"),
        (home.join(".claude"), "stats-cache.json"),
        (home.join(".codex"), "logs_2.sqlite-wal"),
        (home.join(".codex"), "session_index.jsonl"),
    ] {
        std::fs::write(dir.join(name), "1\n2\n3\n").unwrap();
    }
    let noise_delta = watch::diff(&before_noise, &watch::snapshot_roots(&targets));
    assert!(
        noise_delta.is_empty(),
        "CLI 运行时写入被判成外部变更（提示条会一直复现）：{}（变更 {:?}）",
        noise_delta.summary(),
        noise_delta.changed
    );

    // 真身回归：跑一次真实扫描，监听快照不能出现任何差异
    let before = watch::snapshot_roots(&targets);
    let report = hub.scan(&Scope::User).expect("扫描应成功");
    assert!(
        !report.records.is_empty(),
        "沙箱里应有技能记录，否则这条断言没意义"
    );
    let after = watch::snapshot_roots(&targets);
    let delta = watch::diff(&before, &after);
    assert!(
        delta.is_empty(),
        "扫描自己写索引库却被判成外部变更：{}（新增 {:?} / 变更 {:?}）",
        delta.summary(),
        delta.added,
        delta.changed
    );

    // 真正的配置改动仍然要报出来（白名单不能把该看的也挡了）
    std::fs::write(home.join(".claude/settings.json"), "{\"theme\":\"dark\"}").unwrap();
    let after_settings = watch::snapshot_roots(&targets);
    let delta = watch::diff(&after, &after_settings);
    assert!(
        delta
            .added
            .iter()
            .chain(delta.changed.iter())
            .any(|p| p.ends_with("settings.json")),
        "用户手改 settings.json 必须上报，实际 {}",
        delta.summary()
    );
}
