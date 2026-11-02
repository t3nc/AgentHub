//! 真机联网校验（默认不跑）：`cargo test -p agenthub-core --test market_live -- --ignored`
//!
//! 用途是把「解析逻辑」对一遍真实接口 —— 字段位置一旦漂移，这里最先炸。
//! CI 不跑它：读接口在墙外/第三方，且不该让构建依赖对方可用性。
//!
//! 两个源都覆盖：
//! - SkillHub（默认，api.skillhub.cn）额外验证 302 跟随（下载与单文件读取都跳对象存储）；
//! - ClawHub（clawhub.ai）验证保留的可选源没被改坏。

use std::collections::HashSet;

use agenthub_core::AppConfig;
use agenthub_core::market::{self, Registry};
use agenthub_core::model::market::MarketSort;

/// 用第一页返回的游标取第二页，断言两页不重复（游标语义每个源不同，这里只验行为）。
fn assert_second_page_is_new(registry: &dyn Registry, label: &str) {
    let first = registry
        .list_page(MarketSort::Downloads, None, 50)
        .expect("第一页请求失败");
    assert!(first.has_more, "{label} 榜单 50 条之内应该还有下一页");
    let cursor = first.cursor.expect("has_more 为真时必须给出游标");

    let second = registry
        .list_page(MarketSort::Downloads, Some(&cursor), 50)
        .expect("第二页请求失败");
    assert!(!second.items.is_empty(), "{label} 第二页不应为空");

    let seen: HashSet<&str> = first.items.iter().map(|item| item.slug.as_str()).collect();
    let fresh = second
        .items
        .iter()
        .filter(|item| !seen.contains(item.slug.as_str()))
        .count();
    // 榜单是活的（下载量/评分一直在变），翻页边界可能漂移一两条，
    // 前端按 slug 去重兜底；这里只要求「绝大部分是新条目」。
    assert!(
        fresh * 10 >= second.items.len() * 8,
        "{label} 第二页新条目过少：{fresh}/{}",
        second.items.len()
    );
    eprintln!(
        "[live] {label} 分页：第一页 {} 条 → 第二页 {} 条（其中新条目 {fresh}，游标 {}）",
        first.items.len(),
        second.items.len(),
        if second.has_more { "还有" } else { "到底" }
    );
}

fn skillhub() -> Box<dyn Registry> {
    let registry = market::build(&AppConfig::default()).expect("构造默认源失败");
    assert_eq!(registry.base_url(), "https://api.skillhub.cn");
    registry
}

fn clawhub() -> Box<dyn Registry> {
    let mut config = AppConfig::default();
    config.market.source = Some("clawhub".into());
    let registry = market::build(&config).expect("构造 ClawHub 源失败");
    assert_eq!(registry.base_url(), "https://clawhub.ai");
    registry
}

#[test]
#[ignore = "需要联网访问 api.skillhub.cn"]
fn live_skillhub_list_and_detail() {
    let registry = skillhub();
    let items = registry
        .list(MarketSort::Downloads, 5)
        .expect("列表请求失败");
    assert!(!items.is_empty(), "榜单应该有技能");

    let first = &items[0];
    assert!(!first.slug.is_empty());
    assert!(!first.display_name.is_empty());
    assert!(first.downloads > 0, "榜单首条应有下载量");
    eprintln!(
        "[live] SkillHub 列表首条 = {} ({}), @{}, v{:?}, {} 次下载, 官方={}",
        first.display_name, first.slug, first.owner, first.version, first.downloads, first.official
    );

    assert_second_page_is_new(registry.as_ref(), "SkillHub");

    let hits = registry.search("pdf", 5).expect("搜索请求失败");
    assert!(!hits.is_empty(), "搜索 pdf 应该有结果");

    let detail = registry.detail(&hits[0].slug).expect("详情请求失败");
    assert!(
        !detail.description.trim().is_empty(),
        "详情应有正文（SKILL.md 补读或摘要回退）"
    );
    assert!(!detail.files.is_empty(), "详情应带文件清单");
    eprintln!(
        "[live] SkillHub 详情 = v{:?} · {} 个文件 · 审核 {:?}",
        detail.skill.version,
        detail.files.len(),
        detail.moderation_summary
    );
}

#[test]
#[ignore = "需要联网访问 api.skillhub.cn 并下载技能包"]
fn live_skillhub_archive_and_file_text_follow_redirects() {
    let registry = skillhub();
    let items = registry
        .list(MarketSort::Downloads, 10)
        .expect("列表请求失败");
    assert!(!items.is_empty(), "列表不应为空");

    // 下载接口 302 到对象存储：必须跟到底才拿得到 ZIP 魔数
    let mut got_zip = None;
    for target in &items {
        match registry.archive(&target.slug, target.version.as_deref()) {
            Ok(bytes) => {
                assert!(
                    bytes.starts_with(b"PK\x03\x04"),
                    "{} 拿到的不是 ZIP（302 可能没跟随）",
                    target.slug
                );
                got_zip = Some((target.slug.clone(), bytes.len()));
                break;
            }
            Err(agenthub_core::Error::Unsupported(note)) => {
                eprintln!("[live] {} 无 ZIP：{note}", target.slug);
            }
            Err(err) => eprintln!("[live] {} ZIP 失败：{err}", target.slug),
        }
    }
    let (slug, len) = got_zip.expect("榜单里应至少有一个技能能拿到 ZIP");
    eprintln!("[live] SkillHub ZIP：{slug} 包 {len} 字节");

    // 单文件读取同样 302：拿到空串或 HTML 就说明没跟到底
    let text = registry
        .file_text(&slug, "SKILL.md", None)
        .expect("读取 SKILL.md 失败");
    assert!(!text.trim().is_empty(), "SKILL.md 不应为空");
    assert!(
        !text.trim_start().to_ascii_lowercase().starts_with("<html"),
        "拿到的是网页而不是文件内容（302 未跟随）"
    );
    eprintln!(
        "[live] SkillHub SKILL.md 前 40 字：{}",
        text.chars().take(40).collect::<String>()
    );
}

#[test]
#[ignore = "需要联网访问 clawhub.ai"]
fn live_clawhub_list_search_and_detail() {
    let registry = clawhub();
    let items = registry
        .list(MarketSort::Downloads, 5)
        .expect("列表请求失败");
    assert!(!items.is_empty(), "顶榜应该有技能");
    eprintln!(
        "[live] ClawHub 列表首条 = {} (@{}, v{:?}, {} 次下载)",
        items[0].display_name, items[0].owner, items[0].version, items[0].downloads
    );

    assert_second_page_is_new(registry.as_ref(), "ClawHub");

    let hits = registry.search("sql", 5).expect("搜索请求失败");
    assert!(!hits.is_empty(), "搜索 sql 应该有结果");

    let detail = registry.detail(&hits[0].slug).expect("详情请求失败");
    assert!(!detail.description.is_empty(), "详情应有 SKILL.md 正文");
    eprintln!(
        "[live] ClawHub 详情 = v{:?} · license {:?} · {} 个文件 · 审核 {:?}",
        detail.skill.version,
        detail.license,
        detail.files.len(),
        detail.moderation_summary
    );
}
