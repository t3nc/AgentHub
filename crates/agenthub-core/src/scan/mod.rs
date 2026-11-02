pub mod frontmatter;
pub mod tree_hash;

use crate::l10n::{Msg, t};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::adapters::AgentAdapter;
use crate::config::AppConfig;
use crate::error::Result;
use crate::link::link_state;
use crate::model::agent::{AgentId, AgentInstall, Scope, SkillTarget};
use crate::model::skill::{DuplicateGroup, LinkState, SkillRecord, SkillSource};
use crate::paths;
use crate::store::Db;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScanStats {
    pub total: usize,
    pub store_skills: usize,
    pub agent_skills: usize,
    pub linked: usize,
    pub copies: usize,
    pub broken: usize,
    pub readonly: usize,
    pub store_bytes: u64,
    pub wasted_bytes: u64,
    pub shared_skills: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanReport {
    pub generated_at: i64,
    pub scope_label: String,
    pub agents: Vec<AgentInstall>,
    pub targets: Vec<SkillTarget>,
    pub records: Vec<SkillRecord>,
    pub duplicates: Vec<DuplicateGroup>,
    pub stats: ScanStats,
    pub store_root: PathBuf,
    pub duration_ms: u64,
}

struct RawEntry {
    name: String,
    path: PathBuf,
    canonical: PathBuf,
    size: u64,
    mtime: i64,
    files: usize,
    link: LinkState,
    readonly: bool,
}

pub fn scan(
    adapters: &[Box<dyn AgentAdapter>],
    cfg: &AppConfig,
    db: &Db,
    scope: &Scope,
) -> Result<ScanReport> {
    let started = std::time::Instant::now();
    let store_root = cfg.store_root_path();
    paths::ensure_dir(&store_root)?;

    let agents: Vec<AgentInstall> = adapters.iter().map(|a| a.detect()).collect();

    // 同一目录可能被多个 agent 声明（OpenCode 会兼容读 `.claude/skills`）。
    // 只归第一个声明它的 agent：否则同一批文件会被重复计数，还会凭空造出
    // 「自己和自己重复」的重复组，接管时反而去动真实目录。
    let mut seen_dirs: std::collections::HashSet<String> = std::collections::HashSet::new();
    let targets: Vec<SkillTarget> = adapters
        .iter()
        .flat_map(|a| a.skill_targets(scope))
        .filter(|target| seen_dirs.insert(paths::path_key(&target.dir)))
        .collect();

    // 1) 并行遍历各目标目录：只取轻量指纹，不算内容哈希
    let per_target: Vec<(SkillTarget, Vec<RawEntry>)> = targets
        .par_iter()
        .map(|t| (t.clone(), collect(t, &store_root)))
        .collect();

    let store_raw = collect(
        &SkillTarget::new(AgentId::Claude, store_root.clone(), Scope::User),
        &store_root,
    )
    .into_iter()
    .collect::<Vec<_>>();

    // 2) 串行做哈希（命中 SQLite 缓存则跳过内容读取）
    let mut records: Vec<SkillRecord> = Vec::new();
    for entry in &store_raw {
        records.push(build_record(
            entry,
            None,
            SkillSource::Store,
            t!(Msg::LabelSourceStore),
            cfg,
            db,
        )?);
    }
    for (target, entries) in &per_target {
        for entry in entries {
            let source = if entry.readonly {
                SkillSource::Readonly
            } else {
                SkillSource::AgentCopy
            };
            records.push(build_record(
                entry,
                Some(target.agent.clone()),
                source,
                target.scope.label(),
                cfg,
                db,
            )?);
        }
    }

    db.record_skills(&records)?;

    let duplicates = find_duplicates(&records, &store_root);
    let stats = summarize(&records, &duplicates);

    Ok(ScanReport {
        generated_at: paths::now_ts(),
        scope_label: scope.label(),
        agents,
        targets,
        records,
        duplicates,
        stats,
        store_root,
        duration_ms: started.elapsed().as_millis() as u64,
    })
}

fn collect(target: &SkillTarget, store_root: &Path) -> Vec<RawEntry> {
    let mut out = Vec::new();
    let dir = &target.dir;
    if !dir.is_dir() {
        return out;
    }
    let Ok(read) = std::fs::read_dir(crate::platform::long_path(dir)) else {
        return out;
    };
    for entry in read.filter_map(|e| e.ok()) {
        let name = entry.file_name().to_string_lossy().to_string();
        // desktop.ini / Thumbs.db / .DS_Store：跨机一致地跳过，避免扫描结果抖动
        if crate::platform::is_system_junk(&name) {
            continue;
        }
        // .system / .marketplace 等系统项一律归为只读且不改写
        let hidden = name.starts_with('.');
        let path = entry.path();
        if !is_dir_entry(&path) {
            continue;
        }

        // 分类目录（Hermes 的 `skills/apple/apple-notes/SKILL.md`）：自己没有
        // SKILL.md，但子目录里有 —— 展开成多条，name 取子目录名。
        // 隐藏目录也走这条路（如 Codex 的 `.system/imagegen/SKILL.md`）。
        // 只在一级缺 SKILL.md 时多读一层，常见布局仍是原始开销。
        if !has_skill_md(&path) {
            // 只读语义由父目录决定：`.system` 展开后每条仍是只读
            let expanded = expand_category(&path, hidden, target, store_root);
            if !expanded.is_empty() {
                out.extend(expanded);
                continue;
            }
            // 隐藏目录既没有 SKILL.md、也没有技能子目录：锁 / 缓存目录，不是技能
            if hidden {
                continue;
            }
        }

        out.push(make_entry(&path, name, hidden, target, store_root));
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// 子目录是否算一个目录（破损链接也算，交给 `link_state` 判成 Broken）。
///
/// `metadata` 会跟随链接：破损链接（目标被删）在这里直接报错。旧实现因此把破损
/// 链接整个丢掉，导致 `doctor` 永远看不到「链接失效」、`plan_repair` 也就永远
/// 没有修复对象。
fn is_dir_entry(path: &Path) -> bool {
    match std::fs::metadata(crate::platform::long_path(path)) {
        Ok(meta) => meta.is_dir(),
        Err(_) => crate::platform::is_link_like(path),
    }
}

/// 技能目录的标志文件。大小写按平台语义（Windows 不敏感，macOS 区分）。
fn has_skill_md(dir: &Path) -> bool {
    dir.join("SKILL.md").is_file()
}

/// 把「分类目录」展开成它下面每个含 `SKILL.md` 的子目录。
///
/// `hidden` 是父目录的隐藏状态，用于继承只读语义（`.system/*` 一律只读）。
fn expand_category(
    category: &Path,
    hidden: bool,
    target: &SkillTarget,
    store_root: &Path,
) -> Vec<RawEntry> {
    let Ok(read) = std::fs::read_dir(crate::platform::long_path(category)) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in read.filter_map(|e| e.ok()) {
        let name = entry.file_name().to_string_lossy().to_string();
        if crate::platform::is_system_junk(&name) || name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        if is_dir_entry(&path) && has_skill_md(&path) {
            out.push(make_entry(&path, name, hidden, target, store_root));
        }
    }
    out
}

fn make_entry(
    path: &Path,
    name: String,
    hidden: bool,
    target: &SkillTarget,
    store_root: &Path,
) -> RawEntry {
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let fp = tree_hash::fingerprint(path);
    RawEntry {
        name,
        path: path.to_path_buf(),
        canonical,
        size: fp.size,
        mtime: fp.mtime,
        files: fp.files,
        link: link_state(store_root, path),
        readonly: hidden || target.readonly,
    }
}

fn build_record(
    entry: &RawEntry,
    agent: Option<AgentId>,
    source: SkillSource,
    scope_label: String,
    cfg: &AppConfig,
    db: &Db,
) -> Result<SkillRecord> {
    let cached = db
        .cached_hash(&entry.canonical, entry.size, entry.mtime)
        .ok()
        .flatten();
    let hash = match cached {
        Some(h) => Some(h),
        None => {
            let h = tree_hash::tree_hash(&entry.canonical).ok();
            if let Some(ref value) = h {
                let _ = db.put_hash(&entry.canonical, entry.size, entry.mtime, value);
            }
            h
        }
    };
    let fm = frontmatter::parse_skill_dir(&entry.canonical);
    Ok(SkillRecord {
        name: entry.name.clone(),
        path: entry.path.clone(),
        agent,
        scope_label,
        source,
        readonly: entry.readonly,
        description: fm.description,
        tree_hash: hash,
        size_bytes: entry.size,
        file_count: entry.files,
        modified_at: entry.mtime,
        link: entry.link.clone(),
        canonical: entry.canonical.clone(),
        never_upload: cfg
            .translation
            .never_upload
            .iter()
            .any(|n| n == &entry.name),
        agent_local: cfg.is_agent_local(&entry.name),
    })
}

fn find_duplicates(records: &[SkillRecord], store_root: &Path) -> Vec<DuplicateGroup> {
    let mut groups: BTreeMap<&str, Vec<&SkillRecord>> = BTreeMap::new();
    for rec in records {
        // 破损链接没有内容可比，归「修复链接」而不是「去重」；
        // 混进重复组会把整组判成「内容不一致」，让计划变得难以理解。
        //
        // `agent_local`（computer-use 这类各家实现互不通用的技能）同理不分组：
        // 浪费空间、`duplicate_copy` / `duplicate_group` 体检项全都由 duplicates 派生，
        // 在这里剔一次，统计与体检自动跟着干净，不需要再去各处补过滤。
        if rec.readonly || rec.agent_local || rec.link.is_broken() {
            continue;
        }
        groups.entry(rec.name.as_str()).or_default().push(rec);
    }

    let mut out = Vec::new();
    for (name, members) in groups {
        if members.len() < 2 {
            continue;
        }
        let hashes: Vec<Option<&str>> = members.iter().map(|m| m.tree_hash.as_deref()).collect();
        let identical = hashes.iter().all(|h| *h == hashes[0]) && hashes[0].is_some();
        let has_store = members.iter().any(|m| m.source == SkillSource::Store);
        let store_target = paths::normalize_lexical(&store_root.join(name));
        // 只有「有成员尚未指向中央仓库」才值得报告，否则就是已收敛状态
        let needs_action = members.iter().any(|m| {
            if m.source == SkillSource::Store || m.readonly {
                return false;
            }
            match &m.link {
                // 外部链接指向受管目录之外，AgentHub 明确不改写，也就谈不上「待处理」
                LinkState::Foreign { .. } => false,
                LinkState::Broken { .. } => true,
                LinkState::RealDir => true,
                other => match other.target() {
                    Some(target) => !paths::paths_eq(target, &store_target) || !target.exists(),
                    None => true,
                },
            }
        });
        if !needs_action {
            continue;
        }

        let max_size = members.iter().map(|m| m.size_bytes).max().unwrap_or(0);
        let total: u64 = members.iter().map(|m| m.size_bytes).sum();

        let preferred = members
            .iter()
            .find(|m| m.source == SkillSource::Store)
            .or_else(|| {
                members
                    .iter()
                    .filter(|m| !m.readonly)
                    .max_by_key(|m| m.size_bytes)
            })
            .or_else(|| members.first())
            .map(|m| m.canonical.clone())
            .unwrap_or_else(|| store_root.join(name));

        let reason = if has_store {
            t!(Msg::DedupeReasonStoreExists)
        } else if identical {
            t!(Msg::DedupeReasonIdentical)
        } else {
            t!(Msg::DedupeReasonDrift)
        };

        out.push(DuplicateGroup {
            name: name.to_string(),
            members: members.into_iter().cloned().collect(),
            identical,
            wasted_bytes: total.saturating_sub(max_size),
            suggested_source: preferred,
            reason,
        });
    }
    out.sort_by_key(|d| std::cmp::Reverse(d.wasted_bytes));
    out
}

fn summarize(records: &[SkillRecord], duplicates: &[DuplicateGroup]) -> ScanStats {
    let mut stats = ScanStats {
        total: records.len(),
        ..ScanStats::default()
    };
    for rec in records {
        match rec.source {
            SkillSource::Store => {
                stats.store_skills += 1;
                stats.store_bytes += rec.size_bytes;
            }
            SkillSource::Readonly => {
                stats.readonly += 1;
                if rec.agent.is_some() {
                    stats.agent_skills += 1;
                }
            }
            SkillSource::AgentCopy => {
                stats.agent_skills += 1;
                if rec.link.is_linked() {
                    stats.linked += 1;
                } else if rec.link.is_broken() {
                    stats.broken += 1;
                } else {
                    stats.copies += 1;
                }
            }
        }
    }
    stats.wasted_bytes = duplicates.iter().map(|d| d.wasted_bytes).sum();
    // 被 2 个及以上 agent 共用（指向同一 store 目录）的技能数
    let mut usage: BTreeMap<String, usize> = BTreeMap::new();
    for rec in records {
        if rec.link.is_linked() && rec.agent.is_some() {
            *usage.entry(rec.name.clone()).or_default() += 1;
        }
    }
    stats.shared_skills = usage.values().filter(|c| **c >= 2).count();
    stats
}
