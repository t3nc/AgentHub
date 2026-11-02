//! Phase 4：本机 agent 的 token 用量统计。
//!
//! **只读**：不写任何日志文件，也不读取对话正文——解析只取数字、模型名、时间戳
//! 与会话 id。日志里可能有用户 `cat` 过的密钥，所以这条边界不能放松。
//!
//! ## 口径（唯一真源）
//!
//! 各家日志的「输入」语义不同，不能共用一个公式：
//!
//! - **Anthropic 系**（Claude Code）：`input_tokens` **不含**缓存读写，
//!   所以非缓存输入 = `input_tokens`，总量 = 输入 + 缓存读 + 缓存写 + 输出。
//! - **OpenAI 系**（CodeBuddy CLI / IDE、WorkBuddy）：`inputTokens` / `prompt_tokens`
//!   **已包含**缓存命中，所以非缓存输入 = `input − cached`，再加其余三项。
//!
//! 归一后 `input` 恒为「非缓存输入」，前端与 CLI 都只需按 [`TOTALS_NOTE`] 求和。
//!
//! ## 时间
//!
//! 日志时间戳是 UTC，桶键存 **UTC epoch 小时**；「当日」「24 小时」这类范围在聚合层
//! 用 `chrono::Local` 换算。缓存不随用户时区变化而失效。

pub mod ide;
pub mod jsonl;

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::l10n::{self, Msg, t};
use crate::paths;
use crate::platform;
use crate::store::{Db, UsageCacheRow};

/// 解析逻辑的版本号。改动任何解析口径都要 +1：缓存里的旧结果会被整体丢弃重算。
///
/// 5：取不到模型名时的占位从「未知模型」改成稳定哨兵（展示文案走词典，
/// 否则缓存里会留下建缓存那一刻的语言）。
pub const PARSER_VERSION: u32 = 5;
/// 解析器版本在 `app_meta` 里的键。
pub const META_PARSER: &str = "usage_parser";

/// 取不到模型名时的**稳定哨兵**（缓存里存它，展示时再换成 `UsageUnknownModel`）。
///
/// 不能直接存中文：缓存跨语言，存成品文案会让切语言后旧记录保持旧语言。
pub const UNKNOWN_MODEL_ID: &str = "__unknown__";

/// 口径说明的取词入口（注释、UI 脚注、CLI 尾注三处共用，避免各写一份而漂移）。
pub fn totals_note() -> String {
    t!(Msg::UsageTotalsNote)
}

/// 一个数据源的日志格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageFormat {
    /// `~/.claude/projects/**/*.jsonl`：Anthropic 风格 `message.usage`
    ClaudeJsonl,
    /// `~/.codex/sessions/**/*.jsonl`：`token_count` 事件流（累计值）
    CodexJsonl,
    /// CodeBuddy CLI / WorkBuddy 的 `projects/**/*.jsonl`：OpenAI 风格多别名
    OpenAiJsonl,
    /// CodeBuddy IDE 的 `history/<conv>/<req>/index.json`：`requests[].usage`
    CodebuddyIdeIndex,
    /// 登记了但没有可解析的用量（原因写在 `unsupported` 里）
    NoUsage,
}

impl UsageFormat {
    pub fn label(&self) -> String {
        match self {
            // 格式名里的产品名不翻译，只翻描述部分
            UsageFormat::ClaudeJsonl => "Claude JSONL".to_string(),
            UsageFormat::CodexJsonl => t!(Msg::UsageFormatCodex),
            UsageFormat::OpenAiJsonl => t!(Msg::UsageFormatOpenAi),
            UsageFormat::CodebuddyIdeIndex => t!(Msg::UsageFormatCodebuddyIde),
            UsageFormat::NoUsage => t!(Msg::UsageFormatNone),
        }
    }
}

/// 一个数据源。`unsupported` 有值时表示「知道它存在，但拿不到用量」。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageSource {
    /// 稳定标识，同时用作前端品牌图标 key（`claude` / `codebuddy-ide` / …）。
    pub key: String,
    pub label: String,
    pub format: UsageFormat,
    pub roots: Vec<PathBuf>,
    pub exists: bool,
    pub unsupported: Option<String>,
}

impl UsageSource {
    fn parsable(key: &str, label: &str, format: UsageFormat, roots: Vec<PathBuf>) -> UsageSource {
        let exists = roots.iter().any(|root| root.is_dir());
        UsageSource {
            key: key.to_string(),
            label: label.to_string(),
            format,
            roots,
            exists,
            unsupported: None,
        }
    }

    fn unsupported(key: &str, label: &str, reason: Msg) -> UsageSource {
        UsageSource {
            key: key.to_string(),
            label: label.to_string(),
            format: UsageFormat::NoUsage,
            roots: Vec::new(),
            exists: false,
            unsupported: Some(l10n::str_of(reason).to_string()),
        }
    }
}

/// CodeBuddy IDE 的数据根：Windows 在 `%LOCALAPPDATA%`，macOS 在 Application Support。
///
/// 家目录被重定向（`AGENTHUB_HOME` / 测试 override）时改到 `<home>/CodeBuddyExtension/Data`：
/// 这个根不在家目录下，不改道的话沙箱会读到真实 IDE 用量，测出来的数全是脏的。
fn codebuddy_ide_root() -> PathBuf {
    if paths::home_is_overridden() {
        return paths::join_rel(&paths::home_dir(), "CodeBuddyExtension/Data");
    }
    #[cfg(windows)]
    {
        paths::expand("%LOCALAPPDATA%/CodeBuddyExtension/Data")
    }
    #[cfg(target_os = "macos")]
    {
        paths::join_rel(
            &paths::home_dir(),
            "Library/Application Support/CodeBuddyExtension/Data",
        )
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        paths::join_rel(&paths::home_dir(), ".local/share/CodeBuddyExtension/Data")
    }
}

/// 全部数据源（含不可解析项）。
///
/// 顺序即 UI 顺序：能算的在前，拿不到的在后并写明原因——「所有 agent 都统计」的诚实
/// 落地方式是覆盖矩阵，而不是把拿不到的产品显示成 0。
pub fn sources() -> Vec<UsageSource> {
    let home = paths::home_dir();
    vec![
        UsageSource::parsable(
            "claude",
            "Claude Code",
            UsageFormat::ClaudeJsonl,
            vec![paths::join_rel(&home, ".claude/projects")],
        ),
        UsageSource::parsable(
            "codex",
            "Codex",
            UsageFormat::CodexJsonl,
            vec![paths::join_rel(&home, ".codex/sessions")],
        ),
        UsageSource::parsable(
            "codebuddy",
            "CodeBuddy CLI",
            UsageFormat::OpenAiJsonl,
            vec![paths::join_rel(&home, ".codebuddy/projects")],
        ),
        UsageSource::parsable(
            "codebuddy-ide",
            "CodeBuddy IDE",
            UsageFormat::CodebuddyIdeIndex,
            vec![codebuddy_ide_root()],
        ),
        UsageSource::parsable(
            "workbuddy",
            "WorkBuddy",
            UsageFormat::OpenAiJsonl,
            vec![
                paths::join_rel(&home, ".workbuddy/projects"),
                paths::join_rel(&home, ".workbuddy/sessions"),
            ],
        ),
        UsageSource::parsable(
            "workbuddy-ai",
            "WorkBuddy AI",
            UsageFormat::OpenAiJsonl,
            vec![
                paths::join_rel(&home, ".workbuddy-ai/projects"),
                paths::join_rel(&home, ".workbuddy-ai/sessions"),
            ],
        ),
        UsageSource::unsupported("hermes", "Hermes", Msg::UsageWhyHermes),
        UsageSource::unsupported("opencode", "OpenCode", Msg::UsageWhyOpencode),
        UsageSource::unsupported("gemini", "Gemini CLI", Msg::UsageWhyGemini),
        UsageSource::unsupported("trae", "Trae", Msg::UsageWhyTrae),
        UsageSource::unsupported("trae-cn", "Trae CN", Msg::UsageWhyTraeCn),
        UsageSource::unsupported("cursor", "Cursor", Msg::UsageWhyCursor),
        UsageSource::unsupported("copilot", "GitHub Copilot", Msg::UsageWhyCopilot),
        UsageSource::unsupported(
            "claude-desktop",
            "Claude Desktop",
            Msg::UsageWhyClaudeDesktop,
        ),
        UsageSource::unsupported(
            "chatgpt-desktop",
            "ChatGPT / OpenAI desktop",
            Msg::UsageWhyChatgptDesktop,
        ),
        UsageSource::unsupported("manus", "Manus", Msg::UsageWhyManus),
        UsageSource::unsupported("pi", "Pi", Msg::UsageWhyPi),
        UsageSource::unsupported("openwork", "OpenWork", Msg::UsageWhyOpenwork),
    ]
}

/// 一条待聚合的调用记录（聚合前的中间形态）。
#[derive(Debug, Clone)]
pub struct ParsedRow {
    pub hour_utc: i64,
    pub model: String,
    pub provider: Option<String>,
    pub input: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub output: u64,
    /// 跨文件去重指纹；0 表示该来源不需要跨文件去重。
    pub fingerprint: u64,
}

impl ParsedRow {
    pub fn total(&self) -> u64 {
        self.input
            .saturating_add(self.cache_read)
            .saturating_add(self.cache_write)
            .saturating_add(self.output)
    }
}

/// 单个文件的解析结果。
#[derive(Debug, Clone, Default)]
pub struct ParsedFile {
    pub rows: Vec<ParsedRow>,
    pub bad_lines: u64,
}

impl ParsedFile {
    /// 按 `(指纹, 小时, 模型, provider)` 分组的桶。
    ///
    /// 存进缓存的是**去重前**的分组：跨文件去重每次扫描重算，删掉任意一份复制会话
    /// 都不会把用量一起删掉（v3 缓存去重后的结果，删胜出文件就永久丢账）。
    pub fn groups(&self) -> Vec<RowGroup> {
        let mut grouped: BTreeMap<(u64, i64, String, String), UsageBucket> = BTreeMap::new();
        for row in &self.rows {
            let key = (
                row.fingerprint,
                row.hour_utc,
                row.model.clone(),
                provider_key(&row.provider),
            );
            grouped
                .entry(key)
                .or_insert_with(|| UsageBucket {
                    hour_utc: row.hour_utc,
                    model: row.model.clone(),
                    provider: row.provider.clone(),
                    input: 0,
                    cache_read: 0,
                    cache_write: 0,
                    output: 0,
                    calls: 0,
                })
                .add(row);
        }
        grouped
            .into_iter()
            .map(|((fp, _, _, _), bucket)| RowGroup { fp, bucket })
            .collect()
    }
}

/// 一份日志里「同一指纹的一段用量」。`fp == 0` 表示该来源不需要跨文件去重。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RowGroup {
    pub fp: u64,
    pub bucket: UsageBucket,
}

/// 聚合后的小时桶：`(UTC 小时, 模型, provider)` 唯一。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageBucket {
    pub hour_utc: i64,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    pub input: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub output: u64,
    pub calls: u32,
}

impl UsageBucket {
    fn add(&mut self, row: &ParsedRow) {
        self.input = self.input.saturating_add(row.input);
        self.cache_read = self.cache_read.saturating_add(row.cache_read);
        self.cache_write = self.cache_write.saturating_add(row.cache_write);
        self.output = self.output.saturating_add(row.output);
        self.calls = self.calls.saturating_add(1);
    }
}

/// 一个来源的扫描统计。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceStats {
    pub source: UsageSource,
    pub files: usize,
    pub records: u64,
    pub bad_lines: u64,
}

/// 一次统计的整体耗时与缓存命中情况。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScanStats {
    pub files_total: usize,
    pub files_parsed: usize,
    pub files_cached: usize,
    pub bad_lines: u64,
    pub elapsed_ms: u64,
    pub pruned: usize,
}

/// 扫描结果：桶已按 `(来源, 小时, 模型, provider)` 聚合。
pub struct UsageScan {
    pub sources: Vec<SourceStats>,
    pub buckets: Vec<(String, UsageBucket)>,
    pub stats: ScanStats,
}

struct FileTask {
    source: String,
    format: UsageFormat,
    path: PathBuf,
    size: u64,
    mtime: i64,
    /// CodeBuddy IDE 的明细文件没有模型字段，模型来自同会话的清单文件。
    model_hint: Option<String>,
}

fn file_stamp(path: &Path) -> (u64, i64) {
    let Ok(meta) = std::fs::metadata(platform::long_path(path)) else {
        return (0, 0);
    };
    // 用毫秒而不是秒：同一秒内改两次（测试、脚本重跑）在秒级会误判为未变化
    let mtime = meta
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    (meta.len(), mtime)
}

/// 递归收集 `*.jsonl`，跳过子代理日志与消息正文目录。
fn walk_jsonl(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(platform::long_path(root)) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            // 只跳过正文与备份目录。`subagents/` 必须扫：实测（2026-10-31）子代理的
            // message.id 并不出现在父会话文件里，跳过就等于漏账；重复重放由指纹去重挡住。
            if !matches!(name.as_ref(), "messages" | "check-point" | "backups") {
                walk_jsonl(&path, out);
            }
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("jsonl") {
            out.push(path);
        }
    }
}

fn walk_ide_index(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(platform::long_path(root)) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !matches!(
                name.as_ref(),
                "messages" | "check-point" | "backups" | "Public"
            ) {
                walk_ide_index(&path, out);
            }
        } else if entry.file_name().to_string_lossy() == "index.json" && ide::is_detail_index(&path)
        {
            out.push(path);
        }
    }
}

/// 扫描全部来源。`db` 只用于增量缓存，不会写任何 agent 目录。
pub fn scan(db: &Db) -> Result<UsageScan> {
    scan_with(db, sources())
}

/// 按给定来源表扫描（测试可注入沙箱路径，避免碰真实 home）。
pub fn scan_with(db: &Db, sources: Vec<UsageSource>) -> Result<UsageScan> {
    let started = Instant::now();

    // 解析口径变了：整表重算，不让用户手动删库
    if db.meta_get(META_PARSER)?.as_deref() != Some(PARSER_VERSION.to_string().as_str()) {
        db.clear_usage_cache()?;
        db.meta_set(META_PARSER, &PARSER_VERSION.to_string())?;
    }

    let mut tasks: Vec<FileTask> = Vec::new();
    for source in &sources {
        if source.unsupported.is_some() {
            continue;
        }
        let mut files: Vec<PathBuf> = Vec::new();
        for root in &source.roots {
            match source.format {
                UsageFormat::CodebuddyIdeIndex => walk_ide_index(root, &mut files),
                UsageFormat::NoUsage => {}
                _ => walk_jsonl(root, &mut files),
            }
        }
        if matches!(source.format, UsageFormat::CodebuddyIdeIndex) {
            let models = ide::load_models(&files);
            for path in files {
                let (size, mtime) = file_stamp(&path);
                let model_hint =
                    ide::conversation_id(&path).and_then(|conv| models.get(&conv).cloned());
                tasks.push(FileTask {
                    source: source.key.clone(),
                    format: source.format,
                    path,
                    size,
                    mtime,
                    model_hint,
                });
            }
        } else {
            for path in files {
                let (size, mtime) = file_stamp(&path);
                tasks.push(FileTask {
                    source: source.key.clone(),
                    format: source.format,
                    path,
                    size,
                    mtime,
                    model_hint: None,
                });
            }
        }
    }

    // 缓存命中：只查元数据
    let mut cached: Vec<Option<UsageCacheRow>> = Vec::with_capacity(tasks.len());
    let mut keep = BTreeSet::new();
    for task in &tasks {
        keep.insert(paths::path_key(&task.path));
        cached.push(db.usage_cache(&task.path, task.size, task.mtime)?);
    }
    let pruned = db.prune_usage_cache(&keep)?;

    // 未命中的并行解析（纯 CPU / IO，不碰 Db）
    let pending: Vec<usize> = (0..tasks.len()).filter(|i| cached[*i].is_none()).collect();
    let parsed: Vec<ParsedFile> = {
        use rayon::prelude::*;
        pending
            .par_iter()
            .map(|idx| {
                let task = &tasks[*idx];
                match task.format {
                    UsageFormat::ClaudeJsonl => jsonl::parse_claude(&task.path),
                    UsageFormat::CodexJsonl => jsonl::parse_codex(&task.path),
                    UsageFormat::OpenAiJsonl => jsonl::parse_openai_jsonl(&task.path),
                    UsageFormat::CodebuddyIdeIndex => ide::parse_detail(
                        &task.path,
                        task.model_hint.as_deref().unwrap_or(UNKNOWN_MODEL_ID),
                    ),
                    UsageFormat::NoUsage => ParsedFile::default(),
                }
            })
            .collect()
    };
    let mut by_index: HashMap<usize, ParsedFile> = pending.into_iter().zip(parsed).collect();

    // 跨文件去重必须串行：复制会话会把父会话历史重放进自己的文件，
    // 按 mtime 升序处理，用量归到最早那份。
    //
    // 同一毫秒落盘的多个文件（测试、批量复制）在 `mtime` 上完全相等，只按 mtime 排序
    // 会让「谁胜出」取决于 `read_dir` 的枚举顺序 —— macOS 与 Windows 上不一致，
    // 同一份日志两端能算出不同结果。加路径做次级键，保证跨平台可复现。
    let mut order: Vec<usize> = (0..tasks.len()).collect();
    order.sort_by(|a, b| {
        let (left, right) = (&tasks[*a], &tasks[*b]);
        left.mtime
            .cmp(&right.mtime)
            .then_with(|| paths::path_key(&left.path).cmp(&paths::path_key(&right.path)))
    });

    let mut seen: HashSet<u64> = HashSet::new();
    let mut buckets: BTreeMap<(String, i64, String, String), UsageBucket> = BTreeMap::new();
    let mut stats = ScanStats {
        files_total: tasks.len(),
        pruned,
        ..ScanStats::default()
    };
    let mut per_source: BTreeMap<String, (usize, u64, u64)> = BTreeMap::new();

    for idx in order {
        let task = &tasks[idx];
        let entry = per_source.entry(task.source.clone()).or_default();
        entry.0 += 1;

        if let Some(cache) = &cached[idx] {
            merge_groups(
                &mut buckets,
                &mut seen,
                &task.source,
                &decode_groups(&cache.groups),
            );
            entry.1 += cache.records;
            entry.2 += cache.bad_lines;
            stats.files_cached += 1;
            stats.bad_lines += cache.bad_lines;
            continue;
        }

        let Some(file) = by_index.remove(&idx) else {
            continue;
        };
        // 缓存与并入都用同一份分组：缓存存去重前的结果，去重只在本次扫描内做一次
        let groups = file.groups();
        merge_groups(&mut buckets, &mut seen, &task.source, &groups);

        entry.1 += file.rows.len() as u64;
        entry.2 += file.bad_lines;
        stats.files_parsed += 1;
        stats.bad_lines += file.bad_lines;

        let row = UsageCacheRow {
            source: task.source.clone(),
            records: file.rows.len() as u64,
            bad_lines: file.bad_lines,
            groups: encode_groups(&groups),
        };
        db.put_usage_cache(&task.path, &row, task.size, task.mtime)?;
    }

    let source_stats = sources
        .iter()
        .map(|source| {
            let (files, records, bad_lines) =
                per_source.get(&source.key).copied().unwrap_or((0, 0, 0));
            SourceStats {
                source: source.clone(),
                files,
                records,
                bad_lines,
            }
        })
        .collect();

    stats.elapsed_ms = started.elapsed().as_millis() as u64;
    Ok(UsageScan {
        sources: source_stats,
        buckets: buckets
            .into_iter()
            .map(|((source, _, _, _), bucket)| (source, bucket))
            .collect(),
        stats,
    })
}

fn provider_key(provider: &Option<String>) -> String {
    provider.clone().unwrap_or_default()
}

fn merge_bucket(
    out: &mut BTreeMap<(String, i64, String, String), UsageBucket>,
    source: &str,
    bucket: UsageBucket,
) {
    let key = (
        source.to_string(),
        bucket.hour_utc,
        bucket.model.clone(),
        provider_key(&bucket.provider),
    );
    match out.get_mut(&key) {
        Some(existing) => {
            existing.input = existing.input.saturating_add(bucket.input);
            existing.cache_read = existing.cache_read.saturating_add(bucket.cache_read);
            existing.cache_write = existing.cache_write.saturating_add(bucket.cache_write);
            existing.output = existing.output.saturating_add(bucket.output);
            existing.calls = existing.calls.saturating_add(bucket.calls);
        }
        None => {
            out.insert(key, bucket);
        }
    }
}

/// 把一份日志的分组并入结果集，同时把指纹登记进 `seen`。
///
/// 指纹去重是「先到先得」：同一指纹只并入第一次出现的那份（按 mtime + 路径排序），
/// 后续文件里的副本跳过，但它的用量已经在自己的缓存里，删掉任意一份都不丢账。
fn merge_groups(
    out: &mut BTreeMap<(String, i64, String, String), UsageBucket>,
    seen: &mut HashSet<u64>,
    source: &str,
    groups: &[RowGroup],
) {
    for group in groups {
        if group.fp != 0 && !seen.insert(group.fp) {
            continue;
        }
        merge_bucket(out, source, group.bucket.clone());
    }
}

pub fn encode_groups(groups: &[RowGroup]) -> String {
    serde_json::to_string(groups).unwrap_or_else(|_| "[]".into())
}

pub fn decode_groups(raw: &str) -> Vec<RowGroup> {
    serde_json::from_str(raw).unwrap_or_default()
}
