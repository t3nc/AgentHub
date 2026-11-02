//! 单条技能链接策略的覆盖与持久化。
//!
//! 断言的是「解析结果」而不是配置文件长相：缺字段 / 清除覆盖都必须回到平台默认。
//!
//! 注意：`paths::init_overrides` 是 `OnceLock`，同一测试二进制里只有第一次生效。
//! 所以所有需要 `hub()` 的断言都放在一个 `#[test]` 里顺序执行；只测纯函数/序列化的
//! 用例才单独开测试。

mod common;

use agenthub_core::config::AppConfig;
use agenthub_core::link::LinkStrategy;
use agenthub_core::model::agent::{AgentId, Scope};
use agenthub_core::model::change::{ChangeSet, OpAction};

fn create_link_strategies(cs: &ChangeSet) -> Vec<LinkStrategy> {
    cs.ops
        .iter()
        .filter_map(|op| match &op.action {
            OpAction::CreateLink { strategy, .. } => Some(*strategy),
            _ => None,
        })
        .collect()
}

#[test]
fn override_lifecycle_for_dedupe_and_repair() {
    let (home, data) = common::sandbox();
    common::setup(&home);
    let mut hub = common::hub(&home, &data);
    let platform = LinkStrategy::platform_default();

    // ── 1. 没有覆盖时：一律走平台默认（mac/Linux 软链，Windows 目录联接）
    let plan = hub.plan_dedupe(None, None).unwrap();
    let before = create_link_strategies(&plan);
    assert!(!before.is_empty(), "去重计划里应有建链接操作");
    assert!(
        before.iter().all(|s| *s == platform),
        "未设置覆盖时应全部走平台默认 {platform:?}，实际 {before:?}"
    );

    // ── 2. 给 cloudflare 单独设成复制
    let effective = hub
        .set_skill_strategy("cloudflare", Some(LinkStrategy::Copy))
        .unwrap();
    assert_eq!(effective, LinkStrategy::Copy);

    let raw = std::fs::read_to_string(data.join("config.json")).unwrap();
    assert!(
        raw.contains("\"cloudflare\"") && raw.contains("\"copy\""),
        "覆盖应落进 config.json，实际：\n{raw}"
    );

    // ── 3. 重新出计划：cloudflare 用复制，其它技能不受影响
    let plan = hub.plan_dedupe(None, None).unwrap();
    let after = create_link_strategies(&plan);
    assert!(
        !after.is_empty() && after.iter().all(|s| *s == LinkStrategy::Copy),
        "覆盖生效后应为复制，实际 {after:?}"
    );
    assert_eq!(
        hub.strategy_for_skill("only-claude", &AgentId::Claude),
        platform,
        "未设置覆盖的其它技能不受影响"
    );

    // ── 4. 清除覆盖：回到平台默认，配置文件里也不留痕
    let cleared = hub.set_skill_strategy("cloudflare", None).unwrap();
    assert_eq!(cleared, platform);
    let raw = std::fs::read_to_string(data.join("config.json")).unwrap();
    assert!(
        !raw.contains("cloudflare"),
        "清除后不应残留覆盖项，实际：\n{raw}"
    );

    // ── 5. 修复计划同样遵循覆盖
    let store = data.join("store");
    common::write_skill(&store.join("cloudflare"), "cloudflare", "same");
    common::write_skill(&store.join("stale"), "stale", "old");
    let at = home.join(".claude/skills/cloudflare");
    std::fs::remove_dir_all(&at).ok();
    common::link_dir(&store.join("stale"), &at);
    // 目标被删 → 链接破损，但仓库里的 cloudflare 还在，因此可修复
    std::fs::remove_dir_all(store.join("stale")).ok();

    // 回归：破损链接必须能被扫描到（旧实现按 metadata 过滤，会把目标已删的链接整个丢掉）
    let report = hub.scan(&Scope::User).unwrap();
    assert!(
        report
            .records
            .iter()
            .any(|r| r.name == "cloudflare" && r.link.is_broken()),
        "破损链接应出现在扫描结果里，否则 doctor / plan_repair 永远看不到它"
    );

    let plan = hub.plan_repair().unwrap();
    let repair_before = create_link_strategies(&plan);
    assert!(!repair_before.is_empty(), "破损链接应生成修复计划");
    assert!(
        repair_before.iter().all(|s| *s == platform),
        "未覆盖时修复计划走平台默认，实际 {repair_before:?}"
    );

    hub.set_skill_strategy("cloudflare", Some(LinkStrategy::Copy))
        .unwrap();
    let plan = hub.plan_repair().unwrap();
    let repair_after = create_link_strategies(&plan);
    assert!(
        !repair_after.is_empty() && repair_after.iter().all(|s| *s == LinkStrategy::Copy),
        "修复计划也应遵循单条覆盖，实际 {repair_after:?}"
    );

    // 计划能落到盘上才算闭环（copy 策略不依赖平台建链权限）
    let result = hub.apply(&plan, &Scope::User).unwrap();
    assert!(result.applied > 0);
    assert!(at.join("SKILL.md").is_file(), "复制策略应让技能目录可读");
}

#[test]
fn legacy_config_without_override_field_still_loads() {
    let dir = std::env::temp_dir().join(format!("agenthub-cfg-legacy-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.json");
    // 老版本写出的 config.json：没有 skill_strategy_overrides 字段
    std::fs::write(&path, r#"{"store_root":"/tmp/ah-store","backups_keep":3}"#).unwrap();

    let cfg = AppConfig::load_from(&path);
    assert_eq!(cfg.backups_keep, 3, "既有字段照常读取");
    assert!(
        cfg.skill_strategy_overrides.is_empty(),
        "缺字段应退化为空表，而不是整体解析失败"
    );
    assert_eq!(cfg.skill_strategy("cloudflare"), None);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn config_with_bom_still_loads() {
    let dir = std::env::temp_dir().join(format!("agenthub-cfg-bom-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.json");
    // 记事本 / PowerShell 5.1 的 Set-Content 会加 BOM
    std::fs::write(&path, "\u{feff}{\"backups_keep\":4}").unwrap();

    let cfg = AppConfig::load_from(&path);
    assert_eq!(
        cfg.backups_keep, 4,
        "带 BOM 的 config.json 不应退化成默认值"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn override_round_trips_through_json() {
    let mut cfg = AppConfig::default();
    let effective = cfg.set_skill_strategy("cloudflare", Some(LinkStrategy::Junction));
    assert_eq!(effective, LinkStrategy::Junction);

    // 全局默认与单条覆盖同时存在时，单条优先
    cfg.default_strategy = Some(LinkStrategy::Hardlink);
    assert_eq!(
        cfg.skill_strategy("cloudflare"),
        Some(LinkStrategy::Junction)
    );

    let text = serde_json::to_string(&cfg).unwrap();
    let back: AppConfig = serde_json::from_str(&text).unwrap();
    assert_eq!(
        back.skill_strategy("cloudflare"),
        Some(LinkStrategy::Junction),
        "覆盖应能穿过 JSON 往返"
    );
    assert_eq!(back.default_strategy, Some(LinkStrategy::Hardlink));
}
