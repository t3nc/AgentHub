//! Phase 4 解析口径与增量缓存的回归。
//!
//! 一个测试文件里只能有一个建沙箱的用例（`paths::init_overrides` 是 OnceLock），
//! 所以扫描相关的场景全部放在 `scan_cache_and_ranges` 里顺序执行；其余用例只用临时
//! 目录里的夹具文件，不碰 home。

mod common;

use std::fs;

use agenthub_core::api::pricing::{ModelPriceInput, build_prices};
use agenthub_core::api::usage::{UsageRange, report_from};
use agenthub_core::ext::pricing::{self, ModelPrice};
use agenthub_core::ext::usage::{self, ScanStats, UsageBucket, UsageFormat, UsageSource};
use agenthub_core::store::Db;

fn sum<'a>(
    rows: impl Iterator<Item = &'a usage::ParsedRow>,
    pick: impl Fn(&usage::ParsedRow) -> u64,
) -> u64 {
    rows.map(pick).sum()
}

fn claude_line(
    id: &str,
    input: u64,
    output: u64,
    cache_read: u64,
    model: &str,
    side: bool,
) -> String {
    format!(
        r#"{{"type":"assistant","timestamp":"2026-10-31T06:12:00.000Z","sessionId":"s1","isSidechain":{side},"requestedModel":"fallback-model","message":{{"id":"{id}","role":"assistant","model":"{model}","usage":{{"input_tokens":{input},"output_tokens":{output},"cache_read_input_tokens":{cache_read},"cache_creation_input_tokens":7}}}}}}"#
    )
}

#[test]
fn claude_dedupes_message_id_and_counts_sidechain() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("session.jsonl");
    let body = [
        claude_line("a1", 100, 10, 20, "glm-5", false),
        // 同一 message.id 重复写入，只能算一次
        claude_line("a1", 100, 10, 20, "glm-5", false),
        // 子代理（sidechain）是真实花费，照计
        claude_line("a2", 50, 5, 0, "glm-5", true),
        r#"{"type":"user","timestamp":"2026-10-31T06:11:00.000Z","message":{"role":"user"}}"#
            .to_string(),
        // 含 usage 字样但 JSON 坏了：这才是需要计数的坏行
        r#"{"type":"assistant","usage":{ broken"#.to_string(),
    ]
    .join("\n");
    fs::write(&path, body).unwrap();

    let parsed = usage::jsonl::parse_claude(&path);
    assert_eq!(parsed.rows.len(), 2, "重复的 message.id 必须只算一次");
    assert_eq!(parsed.bad_lines, 1, "坏行只计数，不影响整体");
    assert_eq!(sum(parsed.rows.iter(), |r| r.input), 150);
    assert_eq!(sum(parsed.rows.iter(), |r| r.cache_read), 20);
    // Anthropic 系：input_tokens 不含缓存，三项并列
    assert_eq!(sum(parsed.rows.iter(), |r| r.cache_write), 14);
    assert_eq!(sum(parsed.rows.iter(), |r| r.output), 15);
    assert_eq!(parsed.rows[0].model, "glm-5");
    assert_eq!(parsed.rows[0].hour_utc % 3600, 0);
    assert_ne!(
        parsed.rows[0].fingerprint, 0,
        "子代理日志会重放同一消息，Claude 也要指纹做跨文件去重"
    );
    assert_ne!(
        parsed.rows[0].fingerprint, parsed.rows[1].fingerprint,
        "不同 message.id 的指纹必须不同"
    );
}

#[test]
fn codex_uses_cumulative_delta_and_follows_model() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rollout.jsonl");
    let body = [
        r#"{"timestamp":"2026-10-31T06:00:00.000Z","type":"session_meta","payload":{"session_id":"s","model_provider":"relay"}}"#,
        r#"{"timestamp":"2026-10-31T06:00:01.000Z","type":"turn_context","payload":{"model":"gpt-6.1-sol"}}"#,
        r#"{"timestamp":"2026-10-31T06:00:02.000Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":100,"cached_input_tokens":0,"cache_write_input_tokens":0,"output_tokens":10},"last_token_usage":{"input_tokens":100,"cached_input_tokens":0,"cache_write_input_tokens":0,"output_tokens":10}}}}"#,
        r#"{"timestamp":"2026-10-31T06:05:00.000Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":300,"cached_input_tokens":100,"cache_write_input_tokens":5,"output_tokens":30},"last_token_usage":{"input_tokens":200,"cached_input_tokens":100,"cache_write_input_tokens":5,"output_tokens":20}}}}"#,
    ]
    .join("\n");
    fs::write(&path, body).unwrap();

    let parsed = usage::jsonl::parse_codex(&path);
    assert_eq!(parsed.rows.len(), 2);
    // 第一段：全部计入
    assert_eq!(parsed.rows[0].input, 100);
    assert_eq!(parsed.rows[0].output, 10);
    // 第二段：累计值 300 减去上一段 100，且 input 要减掉缓存命中
    assert_eq!(parsed.rows[1].input, 200 - 100);
    assert_eq!(parsed.rows[1].cache_read, 100);
    assert_eq!(parsed.rows[1].cache_write, 5);
    assert_eq!(parsed.rows[1].output, 20);
    // 模型与 provider 都跟随最近一次上下文
    assert_eq!(parsed.rows[1].model, "gpt-6.1-sol");
    assert_eq!(parsed.rows[1].provider.as_deref(), Some("relay"));
}

#[test]
fn codex_restart_resets_baseline() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rollout.jsonl");
    let body = [
        r#"{"timestamp":"2026-10-31T06:00:00.000Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":500,"cached_input_tokens":0,"cache_write_input_tokens":0,"output_tokens":50}}}}"#,
        // 累计值回退 = 会话重启，应视为新起点而不是负数
        r#"{"timestamp":"2026-10-31T06:10:00.000Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":80,"cached_input_tokens":0,"cache_write_input_tokens":0,"output_tokens":8}}}}"#,
    ]
    .join("\n");
    fs::write(&path, body).unwrap();

    let parsed = usage::jsonl::parse_codex(&path);
    assert_eq!(parsed.rows.len(), 2);
    assert_eq!(parsed.rows[1].input, 80);
    assert_eq!(parsed.rows[1].output, 8);
}

#[test]
fn openai_jsonl_subtracts_cached_from_input() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("session.jsonl");
    let line = r#"{"timestamp":1792522730561,"type":"function_call","sessionId":"s","providerData":{"model":"deepseek-v4.1-flash","usage":{"inputTokens":1000,"outputTokens":50,"inputTokensDetails":[{"cached_tokens":800}]}},"message":{"usage":{"input_tokens":1000,"output_tokens":50,"cache_read_input_tokens":800}}}"#;
    fs::write(&path, format!("{line}\n{line}\n")).unwrap();

    let parsed = usage::jsonl::parse_openai_jsonl(&path);
    assert_eq!(parsed.rows.len(), 2);
    // OpenAI 系：input 已含缓存命中，非缓存部分 = 1000 - 800
    assert_eq!(parsed.rows[0].input, 200);
    assert_eq!(parsed.rows[0].cache_read, 800);
    assert_eq!(parsed.rows[0].output, 50);
    assert_eq!(parsed.rows[0].model, "deepseek-v4.1-flash");
    assert_ne!(
        parsed.rows[0].fingerprint, 0,
        "复制会话重放历史，需要指纹去重"
    );
    assert_eq!(
        parsed.rows[0].fingerprint, parsed.rows[1].fingerprint,
        "完全相同的两行必须是同一指纹（上层据此只算一次）"
    );
}

#[test]
fn ide_detail_reads_requests_and_model_from_listing() {
    let dir = tempfile::tempdir().unwrap();
    let history = dir.path().join("Data/u/CodeBuddyIDE/u/history");
    let conv = history.join("conv-1");
    let req = conv.join("req-1");
    fs::create_dir_all(&req).unwrap();
    fs::write(
        conv.join("index.json"),
        r#"{"conversations":[{"id":"conv-1","selectedModelId":"deepseek-v4.1-flash"}]}"#,
    )
    .unwrap();
    let detail = req.join("index.json");
    fs::write(
        &detail,
        r#"{"requests":[{"id":"r1","state":"complete","startedAt":1792226349467,"usage":{"inputTokens":1000,"outputTokens":20,"cacheTokens":900,"cachedWriteTokens":0}}]}"#,
    )
    .unwrap();
    // 清单文件不能被当成明细
    assert!(usage::ide::is_detail_index(&detail));
    assert!(!usage::ide::is_detail_index(&conv.join("index.json")));

    let models = usage::ide::load_models(std::slice::from_ref(&detail));
    assert_eq!(
        models.get("conv-1").map(String::as_str),
        Some("deepseek-v4.1-flash")
    );
    let parsed = usage::ide::parse_detail(&detail, "deepseek-v4.1-flash");
    assert_eq!(parsed.rows.len(), 1);
    assert_eq!(parsed.rows[0].input, 100);
    assert_eq!(parsed.rows[0].cache_read, 900);
    assert_eq!(parsed.rows[0].output, 20);
    assert_eq!(parsed.rows[0].model, "deepseek-v4.1-flash");
}

#[test]
fn price_build_validates_and_costs() {
    let rows = build_prices(&[
        ModelPriceInput {
            model: "glm-5".into(),
            provider: Some("relay".into()),
            input: 3.0,
            output: 15.0,
            cache_read: 0.3,
            cache_write: 3.75,
            currency: None,
        },
        // 同 (model, provider) 后者覆盖前者
        ModelPriceInput {
            model: "glm-5".into(),
            provider: Some("relay".into()),
            input: 4.0,
            output: 16.0,
            cache_read: 0.4,
            cache_write: 4.0,
            currency: Some("cny".into()),
        },
    ])
    .unwrap();
    assert_eq!(rows.len(), 1, "同一 (model, provider) 只保留最后一次");
    assert_eq!(rows[0].input, 4.0);
    assert_eq!(rows[0].currency, "CNY");

    let negative = build_prices(&[ModelPriceInput {
        model: "x".into(),
        provider: None,
        input: -1.0,
        output: 0.0,
        cache_read: 0.0,
        cache_write: 0.0,
        currency: None,
    }]);
    assert!(negative.is_err(), "负单价必须被拒绝");

    // 每百万 token 单价：100 万输入 = 单价本身
    let cost = rows[0].cost(1_000_000, 0, 0, 0);
    assert!((cost - 4.0).abs() < 1e-9, "成本 = 4.0，实际 {cost}");

    let mut with_default = rows.clone();
    with_default.push(ModelPrice {
        model: "glm-5".into(),
        provider: None,
        input: 1.0,
        ..ModelPrice::default()
    });
    // 精确 provider 优先于「默认价」
    let hit = pricing::lookup(&with_default, "glm-5", Some("relay")).unwrap();
    assert_eq!(hit.input, 4.0);
    let fallback = pricing::lookup(&with_default, "glm-5", Some("other")).unwrap();
    assert_eq!(fallback.input, 1.0);
}

#[test]
fn report_aggregates_and_flags_unpriced() {
    let bucket = |hour: i64, model: &str, input: u64| UsageBucket {
        hour_utc: hour,
        model: model.into(),
        provider: None,
        input,
        cache_read: 100,
        cache_write: 10,
        output: 50,
        calls: 2,
    };
    let scan = usage::UsageScan {
        sources: Vec::new(),
        buckets: vec![
            ("claude".into(), bucket(1_790_000_000, "glm-5", 1000)),
            ("codex".into(), bucket(1_790_000_000, "gpt-6.1-sol", 2000)),
        ],
        stats: ScanStats::default(),
    };
    let prices = vec![ModelPrice {
        model: "glm-5".into(),
        provider: None,
        input: 1.0,
        output: 1.0,
        cache_read: 1.0,
        cache_write: 1.0,
        currency: "USD".into(),
        source: Some("manual".into()),
        updated_at: None,
    }];
    let report = report_from(UsageRange::All, scan, &prices);

    assert_eq!(report.totals.input, 3000);
    assert_eq!(report.totals.cache_read, 200);
    assert_eq!(report.totals.calls, 4);
    // 合计 = 非缓存输入 + 命中缓存 + 新建缓存 + 输出
    assert_eq!(report.totals.total, 3000 + 200 + 20 + 100);
    assert_eq!(report.by_agent.len(), 2);
    assert_eq!(report.by_model.len(), 2);
    // gpt-6.1-sol 没有价格：成本只覆盖已定价部分，并标出未定价调用数
    assert_eq!(report.totals.unpriced_calls, 2);
    let expected = (1000.0 + 100.0 + 10.0 + 50.0) / 1_000_000.0;
    assert!((report.totals.cost.unwrap() - expected).abs() < 1e-12);
    // 全部档按天聚合
    assert!(matches!(
        report.granularity,
        agenthub_core::api::usage::Granularity::Day
    ));
}

#[test]
fn ranges_resolve_expected_boundaries() {
    let now = 1_790_000_000i64;
    assert_eq!(UsageRange::All.start_secs(now), None);
    assert_eq!(UsageRange::Last24h.start_secs(now), Some(now - 86_400));
    assert_eq!(UsageRange::Days7.start_secs(now), Some(now - 7 * 86_400));
    assert_eq!(UsageRange::Days30.start_secs(now), Some(now - 30 * 86_400));
    let today = UsageRange::Today.start_secs(now).unwrap();
    assert!(
        today <= now && now - today < 86_400,
        "当日起点必须落在最近 24 小时内"
    );
    assert!(matches!(
        UsageRange::Today.granularity(),
        agenthub_core::api::usage::Granularity::Hour
    ));
    assert_eq!(UsageRange::parse("24h"), UsageRange::Last24h);
    assert_eq!(
        UsageRange::parse("今天"),
        UsageRange::Days7,
        "未知值回落到 7 天"
    );
}

/// 缓存、剪枝与跨文件去重必须整体验证：同一进程只能钉一次 home。
#[test]
fn scan_cache_prune_and_cross_file_dedupe() {
    let (home, data) = common::sandbox();
    let _hub = common::hub(&home, &data);

    let claude_dir = home.join(".claude/projects/p1");
    fs::create_dir_all(&claude_dir).unwrap();
    fs::write(
        claude_dir.join("a.jsonl"),
        claude_line("m1", 100, 10, 0, "glm-5", false),
    )
    .unwrap();

    let openai_dir = home.join(".codebuddy/projects/p1");
    fs::create_dir_all(&openai_dir).unwrap();
    let dup_line = r#"{"timestamp":1792522730561,"providerData":{"model":"m","usage":{"inputTokens":500,"outputTokens":50}}}"#;
    fs::write(openai_dir.join("b.jsonl"), format!("{dup_line}\n")).unwrap();
    let copied_dir = home.join(".codebuddy/projects/p2");
    fs::create_dir_all(&copied_dir).unwrap();
    // 复制会话：同一行被重放到另一个文件，只能算一次
    fs::write(copied_dir.join("c.jsonl"), format!("{dup_line}\n")).unwrap();

    let db = Db::open(&data.join("index.db")).unwrap();
    let sources = vec![
        UsageSource {
            key: "claude".into(),
            label: "Claude Code".into(),
            format: UsageFormat::ClaudeJsonl,
            roots: vec![home.join(".claude/projects")],
            exists: true,
            unsupported: None,
        },
        UsageSource {
            key: "codebuddy".into(),
            label: "CodeBuddy CLI".into(),
            format: UsageFormat::OpenAiJsonl,
            roots: vec![home.join(".codebuddy/projects")],
            exists: true,
            unsupported: None,
        },
    ];

    let first = usage::scan_with(&db, sources.clone()).unwrap();
    assert_eq!(first.stats.files_total, 3);
    assert_eq!(first.stats.files_parsed, 3);
    assert_eq!(first.stats.files_cached, 0);
    let total: u64 = first.buckets.iter().map(|(_, b)| b.input).sum();
    // 100（Claude）+ 500（去重后的唯一一条）= 600
    assert_eq!(total, 600, "跨文件重复的用量只能算一次");

    // 二次扫描：全部命中缓存，不重新解析
    let second = usage::scan_with(&db, sources.clone()).unwrap();
    assert_eq!(second.stats.files_parsed, 0);
    assert_eq!(second.stats.files_cached, 3);
    let total_again: u64 = second.buckets.iter().map(|(_, b)| b.input).sum();
    assert_eq!(total_again, 600, "缓存命中的结果必须与首次一致");

    // 改一个文件：只重解析那一份
    fs::write(
        claude_dir.join("a.jsonl"),
        claude_line("m2", 300, 30, 0, "glm-5", false),
    )
    .unwrap();
    let third = usage::scan_with(&db, sources.clone()).unwrap();
    assert_eq!(third.stats.files_parsed, 1);
    assert_eq!(third.stats.files_cached, 2);

    // 删一个文件：缓存行被剪掉，计数不再包含它
    fs::remove_file(copied_dir.join("c.jsonl")).unwrap();
    let fourth = usage::scan_with(&db, sources.clone()).unwrap();
    assert_eq!(fourth.stats.files_total, 2);
    assert_eq!(fourth.stats.pruned, 1);
    let total_fourth: u64 = fourth.buckets.iter().map(|(_, b)| b.input).sum();
    assert_eq!(total_fourth, 300 + 500);

    // 复制会话回落：c 重新出现时只能算一次，删除「胜出」的 b 也不能把 500 一起删掉。
    // v3 缓存把去重结果固化进每个文件，删掉胜出文件后剩余那份永远是 0（macOS 实测
    // 800 → 300）；v4 缓存存去重前的分组，删任意一份都能保住账。
    fs::write(copied_dir.join("c.jsonl"), format!("{dup_line}\n")).unwrap();
    let fifth = usage::scan_with(&db, sources.clone()).unwrap();
    let total_fifth: u64 = fifth.buckets.iter().map(|(_, b)| b.input).sum();
    assert_eq!(total_fifth, 300 + 500, "复制会话只能算一次");

    fs::remove_file(openai_dir.join("b.jsonl")).unwrap();
    let sixth = usage::scan_with(&db, sources.clone()).unwrap();
    let total_sixth: u64 = sixth.buckets.iter().map(|(_, b)| b.input).sum();
    assert_eq!(total_sixth, 300 + 500, "删掉重复中的任意一份都不应丢账");

    fs::remove_dir_all(home.parent().unwrap()).ok();
}
