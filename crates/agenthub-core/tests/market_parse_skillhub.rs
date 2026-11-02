//! SkillHub（api.skillhub.cn）适配器的离线解析用例。
//!
//! fixture 全部来自真实接口的响应形状（2026-10 实测），
//! 只把内容裁短；不联网，跑 `cargo test` 即生效。

use std::io::{Cursor, Write};

use agenthub_core::config::MarketSource;
use agenthub_core::l10n::{Locale, Msg, text_for};
use agenthub_core::market::http::{FakeHttp, HttpResponse};
use agenthub_core::market::{self, MarketTarget, Registry};
use agenthub_core::model::market::MarketSort;
use zip::write::SimpleFileOptions;

/// 列表：`{code,message,data:{total,skills[]}}` 信封，字段扁平、`tags` 可为 null。
const LIST_JSON: &str = r#"{
  "code": 0,
  "message": "success",
  "data": {
    "total": 212627,
    "skills": [
      {
        "slug": "dev-god",
        "name": "编程大神.Skill",
        "description": "Full-stack coding assistant.",
        "description_zh": "面向复杂真实项目的高阶全栈编程助手。",
        "downloads": 11158061,
        "stars": 63,
        "installs": 0,
        "ownerName": "u_42bbb65f",
        "source": "enterprise",
        "version": "1.2.4",
        "tags": null,
        "updated_at": 1793420337843,
        "publisher": { "name": "阿客实验室", "verified": true },
        "subCategories": [
          { "key": "dev-code-gen", "name": "代码生成" },
          { "key": "dev-bug-fix", "name": "Bug 修复" }
        ]
      },
      {
        "slug": "multi-search-engine",
        "name": "搜索引擎",
        "description": "Cross-engine search.",
        "downloads": 10620295,
        "ownerName": "u_8ee34d77",
        "source": "community",
        "version": "3.0.0",
        "tags": ["search", "检索"],
        "updated_at": 1793420338771
      }
    ]
  }
}"#;

/// 详情：裸对象（无信封），正文 `overviewMd` 实测为空，需要另读 SKILL.md。
const DETAIL_JSON: &str = r#"{
  "slug": "demo-skill",
  "skill": {
    "slug": "demo-skill",
    "displayName": "Demo Skill",
    "summary": "A demo skill for tests.",
    "summary_zh": "测试用的演示技能。",
    "overviewMd": "",
    "source": "community",
    "version": null,
    "tags": { "latest": "1.0.2" },
    "stats": { "downloads": 233019, "installs": 0, "stars": 259, "versions": 3 },
    "updatedAt": 1793420338257,
    "subCategories": [ { "key": "agent-workflow", "name": "工作流编排" } ]
  },
  "latestVersion": { "version": "1.0.2", "changelog": "Initial release" },
  "owner": { "handle": "user_290ac21c", "displayName": "SkillHub" },
  "securityReports": {
    "keen": { "status": "benign", "statusText": "安全，无风险" },
    "sanbu": { "status": "benign", "statusText": "安全，无风险" }
  }
}"#;

const MALICIOUS_JSON: &str = r#"{
  "skill": { "slug": "bad-skill", "displayName": "Bad Skill", "summary": "malware" },
  "latestVersion": { "version": "0.0.1" },
  "owner": { "handle": "evil" },
  "securityReports": {
    "keen": { "status": "malicious", "statusText": "发现恶意行为" },
    "sanbu": { "status": "benign", "statusText": "安全，无风险" }
  }
}"#;

const SUSPICIOUS_JSON: &str = r#"{
  "skill": { "slug": "shady-skill", "displayName": "Shady", "summary": "does something" },
  "latestVersion": { "version": "0.1.0" },
  "securityReports": {
    "keen": { "status": "pending", "statusText": "审核中" }
  }
}"#;

const FILES_JSON: &str = r#"{
  "count": 2,
  "version": "1.0.2",
  "files": [
    { "path": "SKILL.md", "size": 3780, "sha256": "d643" },
    { "path": "references/api.md", "size": 4352, "sha256": "1fd8" }
  ]
}"#;

fn text(body: &str) -> HttpResponse {
    HttpResponse {
        status: 200,
        content_type: "text/markdown".into(),
        body: body.as_bytes().to_vec(),
    }
}

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

fn registry(rules: Vec<(&str, HttpResponse)>) -> Box<dyn Registry> {
    market::build_with(
        FakeHttp::new(rules),
        MarketTarget {
            source: MarketSource::SkillHub,
            base: "https://api.skillhub.test".into(),
        },
    )
    .unwrap()
}

/// 规则按顺序命中，具体路径要排在通配前缀前面。
fn detail_rules(detail: &'static str, skill_md: HttpResponse) -> Vec<(&'static str, HttpResponse)> {
    vec![
        ("/files?version=1.0.2", FakeHttp::json(FILES_JSON)),
        ("/file?path=SKILL.md", skill_md),
        ("/api/v1/skills/demo-skill", FakeHttp::json(detail)),
    ]
}

#[test]
fn list_parses_envelope_and_flat_fields() {
    // 排序参数名必须是 SkillHub 的 sortBy/order/pageSize，拼错会命中不到 fixture
    let registry = registry(vec![(
        "/api/skills?sortBy=downloads&order=desc&pageSize=7",
        FakeHttp::json(LIST_JSON),
    )]);

    let items = registry.list(MarketSort::Downloads, 7).unwrap();
    assert_eq!(items.len(), 2, "信封里的 data.skills 应被展开");

    let first = &items[0];
    assert_eq!(first.slug, "dev-god");
    assert_eq!(first.display_name, "编程大神.Skill");
    assert_eq!(
        first.summary, "面向复杂真实项目的高阶全栈编程助手。",
        "description_zh 应优先于 description"
    );
    assert_eq!(first.owner, "u_42bbb65f");
    assert_eq!(first.version.as_deref(), Some("1.2.4"));
    assert_eq!(first.downloads, 11_158_061);
    assert_eq!(first.installs, 0);
    assert_eq!(first.stars, 63);
    assert_eq!(first.updated_at, Some(1_793_420_337), "毫秒应换算成秒");
    assert_eq!(first.topics, vec!["代码生成", "Bug 修复"]);
    assert!(first.official, "认证发布者应标为官方/认证");
    assert!(!first.suspicious && !first.malware_blocked);
    assert!(first.verdict.is_none(), "列表不含审核信息，不给结论");

    let second = &items[1];
    assert_eq!(
        second.summary, "Cross-engine search.",
        "没有中文时退回英文描述"
    );
    assert_eq!(
        second.topics,
        vec!["search", "检索"],
        "subCategories 缺失时用 tags"
    );
    assert!(!second.official, "source 是开放枚举，不应写死 community");
}

#[test]
fn search_uses_keyword_endpoint() {
    let registry = registry(vec![(
        "/api/skills?keyword=sql%20helper&sortBy=score&order=desc&pageSize=5",
        FakeHttp::json(LIST_JSON),
    )]);

    let hits = registry.search(" sql helper ", 5).unwrap();
    assert_eq!(hits.len(), 2);

    // 这句三语都是固定文案（无占位符），逐语言比对，避免钉死某一种
    let err = registry.search("   ", 5).unwrap_err().to_string();
    let expected: Vec<String> = Locale::ALL
        .iter()
        .map(|locale| text_for(*locale, Msg::ErrSearchQueryEmpty, &[]))
        .collect();
    assert!(
        expected.iter().any(|text| text == &err),
        "空关键词应被拒绝，实际：{err}"
    );
}

#[test]
fn detail_reads_skill_md_and_files() {
    let registry = registry(detail_rules(
        DETAIL_JSON,
        text("---\nname: demo-skill\n---\n\n# Demo\n\nBody text."),
    ));

    let detail = registry.detail("demo-skill").unwrap();
    assert!(
        detail.description.contains("Body text"),
        "正文应来自补读的 SKILL.md"
    );
    assert_eq!(
        detail.skill.version.as_deref(),
        Some("1.0.2"),
        "版本取自 tags/latestVersion"
    );
    assert_eq!(detail.skill.owner, "user_290ac21c");
    assert_eq!(detail.skill.downloads, 233_019);
    assert_eq!(detail.skill.stars, 259);
    assert_eq!(detail.changelog.as_deref(), Some("Initial release"));
    assert!(detail.license.is_none(), "SkillHub 详情没有 license 字段");
    assert_eq!(detail.files.len(), 2);
    assert_eq!(detail.files[0].path, "SKILL.md");
    assert_eq!(
        detail.moderation_summary.as_deref(),
        Some("keen：安全，无风险 · sanbu：安全，无风险")
    );
    assert_eq!(detail.reason_codes, vec!["keen.benign", "sanbu.benign"]);
    assert!(detail.blocked_reason().is_none());
}

#[test]
fn detail_falls_back_to_summary_when_skill_md_missing() {
    let registry = registry(vec![
        ("/files?version=1.0.2", FakeHttp::json(FILES_JSON)),
        ("/file?path=SKILL.md", FakeHttp::status(404)),
        ("/api/v1/skills/demo-skill", FakeHttp::json(DETAIL_JSON)),
    ]);

    let detail = registry.detail("demo-skill").unwrap();
    assert_eq!(detail.description, "测试用的演示技能。");
}

#[test]
fn malicious_report_blocks_install() {
    let registry = registry(vec![(
        "/api/v1/skills/bad-skill",
        FakeHttp::json(MALICIOUS_JSON),
    )]);
    let detail = registry.detail("bad-skill").unwrap();
    assert!(detail.skill.malware_blocked);
    assert_eq!(detail.skill.verdict.as_deref(), Some("malicious"));
    assert!(detail.blocked_reason().is_some(), "恶意技能必须拒装");
}

#[test]
fn unknown_report_status_is_flagged_suspicious() {
    let registry = registry(vec![(
        "/api/v1/skills/shady-skill",
        FakeHttp::json(SUSPICIOUS_JSON),
    )]);
    let detail = registry.detail("shady-skill").unwrap();
    assert!(detail.skill.suspicious, "非 benign 的状态应视为可疑");
    assert!(!detail.skill.malware_blocked);
    assert!(
        detail.blocked_reason().is_none(),
        "可疑允许安装，只是给警告"
    );
}

#[test]
fn archive_accepts_zip_and_rejects_pages() {
    let zip = build_zip(&[("SKILL.md", "---\nname: demo-skill\n---\n")]);
    let registry = registry(vec![
        ("/api/v1/download?slug=zip-skill", FakeHttp::zip(zip)),
        (
            "/api/v1/download?slug=web-skill",
            FakeHttp::json(r#"{"error":"redirect"}"#),
        ),
    ]);

    let bytes = registry.archive("zip-skill", None).unwrap();
    assert!(bytes.starts_with(b"PK\x03\x04"));

    let err = registry.archive("web-skill", None).unwrap_err();
    assert!(
        matches!(err, agenthub_core::Error::Unsupported(_)),
        "非 ZIP 应触发逐文件降级，实际 {err}"
    );
}

#[test]
fn business_code_error_is_surfaced() {
    let registry = registry(vec![(
        "/api/skills?sortBy=score&order=desc&pageSize=5",
        FakeHttp::json(r#"{"code": 4290, "message": "quota exceeded", "data": null}"#),
    )]);

    let err = registry
        .list(MarketSort::Recommended, 5)
        .unwrap_err()
        .to_string();
    assert!(err.contains("quota exceeded"), "{err}");
}

#[test]
fn list_page_uses_page_cursor_and_total() {
    let registry = registry(vec![
        (
            "/api/skills?sortBy=downloads&order=desc&pageSize=5&page=2",
            FakeHttp::json(
                r#"{"code":0,"data":{"total":12,"skills":[{"slug":"p2-skill","name":"P2"}]}}"#,
            ),
        ),
        (
            "/api/skills?sortBy=downloads&order=desc&pageSize=5&page=1",
            FakeHttp::json(
                r#"{"code":0,"data":{"total":12,"skills":[{"slug":"p1-skill","name":"P1"}]}}"#,
            ),
        ),
    ]);

    let first = registry.list_page(MarketSort::Downloads, None, 5).unwrap();
    assert_eq!(first.items[0].slug, "p1-skill");
    assert!(first.has_more, "5 * 1 = 5 < total 12");
    assert_eq!(first.cursor.as_deref(), Some("2"), "游标是下一页页码");

    let second = registry
        .list_page(MarketSort::Downloads, first.cursor.as_deref(), 5)
        .unwrap();
    assert_eq!(second.items[0].slug, "p2-skill");
    assert_eq!(second.cursor.as_deref(), Some("3"), "10 < 12 还有一页");
    assert!(second.has_more);
}

#[test]
fn search_page_passes_keyword_and_page() {
    let registry = registry(vec![(
        "/api/skills?keyword=pdf&sortBy=score&order=desc&pageSize=5&page=2",
        FakeHttp::json(
            r#"{"code":0,"data":{"total":9,"skills":[{"slug":"pdf-2","name":"PDF 2"}]}}"#,
        ),
    )]);

    let page = registry.search_page("pdf", Some("2"), 5).unwrap();
    assert_eq!(page.items[0].slug, "pdf-2");
    assert!(!page.has_more, "2 * 5 = 10 >= total 9，到底了");
    assert!(page.cursor.is_none());
}

#[test]
fn files_uses_dedicated_endpoint() {
    let registry = registry(vec![("/files?version=1.0.2", FakeHttp::json(FILES_JSON))]);
    let files = registry.files("demo-skill", Some("1.0.2")).unwrap();
    assert_eq!(files.len(), 2);
    assert_eq!(files[1].path, "references/api.md");
    assert_eq!(files[1].size, 4352);
    assert!(files[1].content_type.is_none(), "该接口不返回 contentType");
}
