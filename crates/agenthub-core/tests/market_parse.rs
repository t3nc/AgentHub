mod common;

use std::io::{Cursor, Write};

use agenthub_core::config::MarketSource;
use agenthub_core::l10n::{Locale, Msg, text_for};
use agenthub_core::market::http::FakeHttp;
use agenthub_core::market::{self, MarketTarget, Registry};
use agenthub_core::model::agent::{AgentId, Scope};
use agenthub_core::model::change::{FileOp, OpAction};
use agenthub_core::model::market::{MarketInstallInput, MarketSort};
use zip::write::SimpleFileOptions;

/// 搜索接口的真实形状：`results[]`，`downloads` 铺在顶层，可疑标记在 `native.skill` 里。
const SEARCH_JSON: &str = r#"{
  "results": [
    {
      "slug": "sql-helper",
      "displayName": "SQL Helper",
      "summary": "Write safer SQL queries.\nIncludes review checklist.",
      "downloads": 2193,
      "updatedAt": 1793379562875,
      "ownerHandle": "cargo-ai",
      "official": true,
      "native": { "skill": { "isSuspicious": false, "stats": { "installs": 19, "stars": 4 } } }
    },
    {
      "slug": "shady-skill",
      "displayName": "Shady",
      "summary": "does something",
      "downloads": 5,
      "owner": { "handle": "anon" },
      "native": { "skill": { "isSuspicious": true } }
    }
  ]
}"#;

/// 列表接口的形状：`items[]` + `stats` 嵌套 + `latestVersion`。
const LIST_JSON: &str = r#"{
  "items": [
    {
      "slug": "demo-skill",
      "displayName": "Demo Skill",
      "summary": "A demo skill for tests.",
      "ownerHandle": "tester",
      "stats": { "downloads": 10, "installs": 2, "stars": 1 },
      "latestVersion": { "version": "1.2.0" },
      "updatedAt": 1793377006822
    }
  ],
  "nextCursor": "abc"
}"#;

const DETAIL_JSON: &str = r#"{
  "skill": {
    "slug": "demo-skill",
    "displayName": "Demo Skill",
    "summary": "A demo skill for tests.",
    "description": "---\nname: demo-skill\n---\n\n# Demo\n\nBody text.",
    "topics": ["Testing"],
    "stats": { "downloads": 10, "installs": 2, "stars": 1 },
    "updatedAt": 1793377006822
  },
  "latestVersion": { "version": "1.2.0", "license": "MIT", "changelog": "initial" },
  "owner": { "handle": "tester", "displayName": "Tester" },
  "moderation": { "isSuspicious": false, "isMalwareBlocked": false, "verdict": "clean", "summary": "Review: llm_review", "reasonCodes": ["review.llm_review"] },
  "links": { "canonical": "/tester/skills/demo-skill" },
  "files": [
    { "path": "SKILL.md", "size": 40, "contentType": "text/markdown" },
    { "path": "references/a.md", "size": 10, "contentType": "text/markdown" }
  ]
}"#;

const BLOCKED_JSON: &str = r#"{
  "skill": { "slug": "bad-skill", "displayName": "Bad Skill", "summary": "malware" },
  "latestVersion": { "version": "0.0.1" },
  "owner": { "handle": "evil" },
  "moderation": { "isSuspicious": true, "isMalwareBlocked": true, "verdict": "malicious" }
}"#;

fn build_zip(entries: &[(&str, &str)]) -> Vec<u8> {
    let mut buffer = Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut buffer);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (name, body) in entries {
            writer.start_file(*name, options).unwrap();
            writer.write_all(body.as_bytes()).unwrap();
        }
        writer.finish().unwrap();
    }
    buffer.into_inner()
}

fn fake_registry() -> Box<dyn Registry> {
    let zip = build_zip(&[
        (
            "SKILL.md",
            "---\nname: demo-skill\ndescription: demo\n---\n\n# Demo\n",
        ),
        ("references/a.md", "hello"),
    ]);
    let rules = vec![
        ("/api/v1/search", FakeHttp::json(SEARCH_JSON)),
        ("/api/v1/skills?sort=", FakeHttp::json(LIST_JSON)),
        ("/api/v1/download", FakeHttp::zip(zip)),
        ("/api/v1/skills/bad-skill", FakeHttp::json(BLOCKED_JSON)),
        (
            "/api/v1/skills/demo-skill/versions/",
            FakeHttp::json(DETAIL_JSON),
        ),
        ("/api/v1/skills/demo-skill", FakeHttp::json(DETAIL_JSON)),
    ];
    clawhub(FakeHttp::new(rules))
}

/// ClawHub 适配器的离线实例（fixture 用的都是 ClawHub 的响应形状）。
fn clawhub(http: FakeHttp) -> Box<dyn Registry> {
    market::build_with(
        http,
        MarketTarget {
            source: MarketSource::ClawHub,
            base: "https://fake.local".into(),
        },
    )
    .unwrap()
}

fn has_move(ops: &[FileOp]) -> bool {
    ops.iter()
        .any(|op| matches!(op.action, OpAction::MoveIntoStore { .. }))
}

fn link_targets(ops: &[FileOp]) -> Vec<String> {
    ops.iter()
        .filter_map(|op| match &op.action {
            OpAction::CreateLink { at, .. } => Some(agenthub_core::paths::path_key(at)),
            _ => None,
        })
        .collect()
}

#[test]
fn market_search_and_detail_are_parsed() {
    let registry = fake_registry();

    let hits = registry.search("sql", 10).unwrap();
    assert_eq!(hits.len(), 2);
    assert_eq!(
        hits[0].slug, "sql-helper",
        "必须保留服务端顺序：分页是把新一页追加到尾部，重排会让顺序错乱"
    );
    let shady = hits
        .iter()
        .find(|skill| skill.slug == "shady-skill")
        .unwrap();
    assert!(shady.suspicious, "native.skill.isSuspicious 应被识别");
    let sql = hits.iter().find(|s| s.slug == "sql-helper").unwrap();
    assert_eq!(sql.display_name, "SQL Helper");
    assert_eq!(sql.downloads, 2193, "顶层 downloads 应被读到");
    assert_eq!(sql.installs, 19, "嵌套 stats.installs 应被读到");
    assert_eq!(sql.owner, "cargo-ai");
    assert!(sql.official);
    assert!(!sql.summary.contains('\n'), "摘要应压成单行");
    assert_eq!(sql.updated_at, Some(1793379562), "毫秒应换算成秒");

    let listed = registry.list(MarketSort::Downloads, 10).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].slug, "demo-skill");
    assert_eq!(listed[0].version.as_deref(), Some("1.2.0"));
    assert_eq!(listed[0].downloads, 10);

    let detail = registry.detail("demo-skill").unwrap();
    assert!(
        detail.description.contains("Body text"),
        "详情应带 SKILL.md 正文"
    );
    assert_eq!(detail.license.as_deref(), Some("MIT"));
    assert_eq!(detail.files.len(), 2);
    assert_eq!(detail.reason_codes, vec!["review.llm_review"]);
    assert_eq!(
        detail.homepage.as_deref(),
        Some("https://fake.local/tester/skills/demo-skill"),
        "相对链接应补成绝对地址"
    );
    assert!(detail.blocked_reason().is_none());

    // 没指定版本时，files() 走详情（省一次请求）
    let files = registry.files("demo-skill", None).unwrap();
    assert_eq!(files.len(), 2);

    let blocked = registry.detail("bad-skill").unwrap();
    assert!(blocked.blocked_reason().is_some(), "恶意技能应被拦截");
}

#[test]
fn install_plan_uses_store_and_links() {
    let (home, data) = common::sandbox();
    common::setup(&home);
    let hub = common::hub(&home, &data);
    let registry = fake_registry();

    // 本机可安装目标：沙箱里 .claude / .codex / .hermes / .gemini 都有目录
    let agents = hub.market_agents();
    let claude = agents.iter().find(|a| a.id == AgentId::Claude).unwrap();
    assert!(claude.available, "沙箱里 claude 应可用");
    assert!(claude.skills_dir.is_some());

    let input = MarketInstallInput {
        slug: "demo-skill".into(),
        version: None,
        agents: vec![AgentId::Claude, AgentId::Codex],
    };
    let plan = hub
        .plan_market_install_with(registry.as_ref(), &input)
        .unwrap();
    assert!(has_move(&plan.ops), "必须把缓存目录搬进中央仓库");
    assert_eq!(link_targets(&plan.ops).len(), 2, "两个 agent 各一条链接");
    // 文案随界面语言变化（CI 的 locale 是 en_US），这里断言结构：
    // 必须有一条同时说明「包内 2 个文件」「56 B 体积」和「来自 ZIP」。
    assert!(
        plan.notes
            .iter()
            .any(|n| n.contains('2') && n.contains("56 B") && n.contains("ZIP")),
        "应报告包内文件数与体积，实际 {:?}",
        plan.notes
    );

    // 恶意技能：直接拒绝，不产生计划（放在安装之前：这两条都不依赖落盘，
    // 也就不会因为本机目录联接不可遍历而掩盖掉真正的断言）。
    let err = hub
        .plan_market_install_with(
            registry.as_ref(),
            &MarketInstallInput {
                slug: "bad-skill".into(),
                version: None,
                agents: vec![AgentId::Claude],
            },
        )
        .unwrap_err();
    // 文案随界面语言走（CI 是 en_US），断言「点名了哪个技能」而不是中文词
    assert!(
        err.to_string().contains("Bad Skill"),
        "拒绝安装时应点名是哪个技能：{err}"
    );
    assert!(
        matches!(err, agenthub_core::Error::Unsupported(_)),
        "恶意技能应报 Unsupported：{err}"
    );

    // 没选 agent：报错而不是默默装到某处
    let err = hub
        .plan_market_install_with(
            registry.as_ref(),
            &MarketInstallInput {
                slug: "demo-skill".into(),
                version: None,
                agents: vec![],
            },
        )
        .unwrap_err();
    // 这句在三语里都是固定文案，用 text_for 逐语言比对，避免钉死某一种
    let expected: Vec<String> = Locale::ALL
        .iter()
        .map(|locale| text_for(*locale, Msg::ErrMarketSelectAgent, &[]))
        .collect();
    assert!(
        expected.iter().any(|text| text == &err.to_string()),
        "没选 agent 时应提示至少选一个，实际：{err}"
    );

    let result = hub.apply(&plan, &Scope::User).unwrap();
    let store_skill = data.join("store/demo-skill");
    assert!(store_skill.join("SKILL.md").is_file(), "仓库应有真身");
    assert!(
        store_skill.join("references/a.md").is_file(),
        "子目录文件应保留"
    );
    for link in [
        home.join(".claude/skills/demo-skill"),
        home.join(".codex/skills/demo-skill"),
    ] {
        common::assert_linked(&link);
        assert!(link.join("SKILL.md").is_file(), "{} 应可读", link.display());
    }

    // 缓存目录已被搬走，不应留下垃圾
    let cache = data.join("market/cache");
    let leftover = std::fs::read_dir(&cache)
        .map(|read| read.filter_map(|e| e.ok()).count())
        .unwrap_or(0);
    assert_eq!(leftover, 0, "安装后缓存目录应清空");

    // 回滚：真身与链接都撤销
    hub.rollback(&result.batch_id).unwrap();
    assert!(!store_skill.exists(), "回滚后仓库应清空");
    assert!(!home.join(".claude/skills/demo-skill").exists());
}

const PAGE1_JSON: &str = r#"{
  "items": [
    { "slug": "a-skill", "displayName": "A", "summary": "a", "stats": { "downloads": 3 } },
    { "slug": "b-skill", "displayName": "B", "summary": "b", "stats": { "downloads": 2 } }
  ],
  "nextCursor": "{\"v\":1,\"index\":\"by_downloads\",\"key\":[3,2]}"
}"#;

const PAGE2_JSON: &str = r#"{
  "items": [
    { "slug": "c-skill", "displayName": "C", "summary": "c", "stats": { "downloads": 1 } }
  ],
  "nextCursor": ""
}"#;

#[test]
fn list_page_follows_cursor_and_keeps_order() {
    // 规则按顺序命中：带 cursor 的那条必须排前面
    let registry = clawhub(FakeHttp::new(vec![
        (
            "/api/v1/skills?sort=downloads&limit=2&cursor=",
            FakeHttp::json(PAGE2_JSON),
        ),
        (
            "/api/v1/skills?sort=downloads&limit=2",
            FakeHttp::json(PAGE1_JSON),
        ),
    ]));

    let first = registry.list_page(MarketSort::Downloads, None, 2).unwrap();
    assert_eq!(first.items.len(), 2);
    assert_eq!(first.items[0].slug, "a-skill", "不能按 slug 重排");
    assert!(first.has_more);
    let cursor = first.cursor.expect("第一页应给出 nextCursor");

    let second = registry
        .list_page(MarketSort::Downloads, Some(&cursor), 2)
        .unwrap();
    assert_eq!(second.items.len(), 1);
    assert_eq!(second.items[0].slug, "c-skill");
    assert!(!second.has_more, "nextCursor 为空 = 到底");
    assert!(second.cursor.is_none());
}

#[test]
fn search_page_simulates_offset_because_search_has_no_cursor() {
    // 第二批要把 offset 折进 limit：offset=1 + limit=10 → limit=11
    let registry = clawhub(FakeHttp::new(vec![(
        "/api/v1/search?q=sql&limit=11",
        FakeHttp::json(SEARCH_JSON),
    )]));

    let page = registry.search_page("sql", Some("offset:1"), 10).unwrap();
    assert_eq!(page.items.len(), 1, "应跳过已取的第 1 条");
    assert_eq!(page.items[0].slug, "shady-skill");
    assert!(!page.has_more, "只拿到 2 条（未满 11），说明没有下一页");

    // 游标到顶后不再请求
    let capped = registry.search_page("sql", Some("offset:200"), 10).unwrap();
    assert!(capped.items.is_empty());
    assert!(!capped.has_more);
    assert!(capped.cursor.is_none());
}

#[test]
fn http_errors_map_to_readable_messages() {
    let rules = vec![
        ("/api/v1/search", FakeHttp::status(429)),
        ("/api/v1/skills?sort=", FakeHttp::status(404)),
    ];
    let registry = clawhub(FakeHttp::new(rules));

    // 断言错误变体而不是中文词：文案随界面语言走，映射关系才是这里要保的东西
    let err = registry.search("x", 5).unwrap_err();
    assert!(
        matches!(err, agenthub_core::Error::Other(_)),
        "429 应映射成可读的限流说明，实际：{err}"
    );
    let err = registry.list(MarketSort::Recommended, 5).unwrap_err();
    assert!(
        matches!(err, agenthub_core::Error::NotFound(_)),
        "404 应映射成 NotFound，实际：{err}"
    );
}
