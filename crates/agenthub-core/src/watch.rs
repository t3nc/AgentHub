//! 外部变更感知：轻量快照 + 差异比较。
//!
//! 目标不是实时索引，而是回答一个问题：**外部工具动过 agent 目录 / 中央仓库 / 配置吗？**
//! 有变化就让界面提示「重新扫描」，而不是自己偷偷重扫一遍。
//!
//! 设计取舍：
//! - **只做一级目录**：技能目录内部的文件改动不触发（那种改动量小，手动重扫即可），
//!   同时也避开 Windows junction / reparse point 递归带来的事件风暴。
//! - **目录也用 mtime 参与比较**：新增或删除子项会更新父目录 mtime，能覆盖「装了新技能」。
//! - 纯函数、不依赖 Tauri：CLI 与测试可以直接用。

use crate::l10n::{Msg, t};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// 单个条目（某个受管根下的一级子项）的指纹。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// 文件字节数；目录记为 0
    pub size: u64,
    /// 最后修改时间（Unix 毫秒）
    ///
    /// 用毫秒而不是秒：配置文件常被「同长度替换」（`model = "a"` → `model = "b"`），
    /// 秒级精度会把这种改动吞掉。
    pub mtime: i64,
    pub is_dir: bool,
}

/// 所有受管根的一级条目快照：`绝对路径 -> 指纹`。
pub type Snapshot = BTreeMap<PathBuf, Entry>;

/// 一个受管根的监听范围。
///
/// 技能目录与中央仓库关注全部一级子项（`only = None`）；agent 配置目录只关注
/// 真正的配置文件（`Some(names)`）。后者是关键：`~/.claude` / `~/.codex` 里混着
/// 大量 CLI 运行时文件（`history.jsonl`、`logs_2.sqlite-wal` …），CLI 一在跑就
/// 持续写，整目录监听会让「外部有变更」永远消不掉。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchRoot {
    pub path: PathBuf,
    /// `None` = 一级子项全部关注；`Some(names)` = 只关注这些文件名（大小写不敏感）
    #[serde(default)]
    pub only: Option<Vec<String>>,
}

impl WatchRoot {
    /// 关注该根下全部一级子项。
    pub fn all(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            only: None,
        }
    }

    /// 只关注名单里的文件名（名单会统一成小写并去重排序）。
    pub fn whitelist(path: impl Into<PathBuf>, names: impl IntoIterator<Item = String>) -> Self {
        let mut names: Vec<String> = names
            .into_iter()
            .map(|name| name.to_ascii_lowercase())
            .filter(|name| !name.is_empty())
            .collect();
        names.sort();
        names.dedup();
        Self {
            path: path.into(),
            only: Some(names),
        }
    }

    /// 合并同一个根上的两份规格：任一方「全部关注」则结果为全部关注。
    pub fn merge(&mut self, other: Option<Vec<String>>) {
        match (&mut self.only, other) {
            (None, _) => {}
            (slot @ Some(_), None) => *slot = None,
            (Some(mine), Some(mut theirs)) => {
                mine.append(&mut theirs);
                mine.sort();
                mine.dedup();
            }
        }
    }

    /// 该根是否关心这个文件名。
    pub fn accepts(&self, name: &str) -> bool {
        match &self.only {
            None => true,
            Some(names) => {
                let lower = name.to_ascii_lowercase();
                names.iter().any(|known| known == &lower)
            }
        }
    }
}

/// 一份快照相对上一份的差异。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchDelta {
    /// 新增的条目
    pub added: Vec<PathBuf>,
    /// 被删掉的条目
    pub removed: Vec<PathBuf>,
    /// 指纹变了的条目（大小或 mtime）
    pub changed: Vec<PathBuf>,
}

impl WatchDelta {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.changed.is_empty()
    }

    /// 供命令行/日志用的单行摘要。
    pub fn summary(&self) -> String {
        t!(
            Msg::WatchDeltaSummary,
            added = self.added.len(),
            removed = self.removed.len(),
            changed = self.changed.len()
        )
    }
}

/// 本应用自己的产物（索引库与临时文件）。
///
/// 正常情况下数据根不在监听列表里，但用户可以把中央仓库指到数据根内部，
/// 那时 `index.db` 的写入会落在受管根的一级条目上——那是「自己在写」，
/// 不是外部变更，必须排除，否则点「重扫」会把自己刚写的东西又报一遍。
fn is_own_artifact(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.starts_with("index.db") || lower.ends_with(".tmp") || lower.ends_with(".lock")
}

/// agent CLI 的运行时产物：会话历史、统计缓存、sqlite 日志库、日志/pid 文件。
///
/// 配置根改成白名单之后这些本来就不在名单里，这里再兜一层——用户把技能目录指向
/// 别处、或 CLI 把状态文件落到技能目录一级时，也不会把提示条刷成常亮。
fn is_runtime_noise(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".log") || lower.ends_with(".pid") || lower.ends_with(".sock") {
        return true;
    }
    if lower.contains(".sqlite")
        || lower.ends_with(".db")
        || lower.ends_with(".db-wal")
        || lower.ends_with(".db-shm")
    {
        return true;
    }
    matches!(
        lower.as_str(),
        "history.jsonl"
            | "session_index.jsonl"
            | "stats-cache.json"
            | "daemon.log"
            | "daemon.status.json"
            | "daemon.lock"
    )
}

fn mtime_of(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// 取某个受管根下的一级条目快照（不递归），不做白名单过滤。
///
/// 根目录不存在时返回空表：调用方在「目录刚被创建」的场景下会先收到空快照，
/// 下一次事件就能看到全部新增项，不会漏报。
pub fn snapshot(root: &Path) -> Snapshot {
    collect(root, None)
}

/// 按监听规格取一级条目快照：白名单根只保留名单内的文件名。
pub fn snapshot_root(root: &WatchRoot) -> Snapshot {
    collect(&root.path, Some(root))
}

fn collect(root: &Path, spec: Option<&WatchRoot>) -> Snapshot {
    let mut out = Snapshot::new();
    let Ok(read) = std::fs::read_dir(crate::platform::long_path(root)) else {
        return out;
    };
    for entry in read.filter_map(|e| e.ok()) {
        let name = entry.file_name().to_string_lossy().to_string();
        // 与扫描保持一致：系统垃圾文件不算变更，避免跨机同步盘反复触发
        if crate::platform::is_system_junk(&name)
            || is_own_artifact(&name)
            || is_runtime_noise(&name)
        {
            continue;
        }
        if let Some(spec) = spec
            && !spec.accepts(&name)
        {
            continue;
        }
        let path = root.join(&name);
        let Ok(meta) = std::fs::symlink_metadata(crate::platform::long_path(&path)) else {
            continue;
        };
        let is_dir = meta.is_dir();
        out.insert(
            path,
            Entry {
                size: if is_dir { 0 } else { meta.len() },
                mtime: mtime_of(&meta),
                is_dir,
            },
        );
    }
    out
}

/// 合并多个受管根的一级快照。
pub fn snapshot_all(roots: &[PathBuf]) -> Snapshot {
    let mut out = Snapshot::new();
    for root in roots {
        out.extend(snapshot(root));
    }
    out
}

/// 按监听规格合并多个根的一级快照。
pub fn snapshot_roots(roots: &[WatchRoot]) -> Snapshot {
    let mut out = Snapshot::new();
    for root in roots {
        out.extend(snapshot_root(root));
    }
    out
}

/// 比较两份快照。只报告「路径级」差异，不推断谁改了什么内容。
pub fn diff(prev: &Snapshot, next: &Snapshot) -> WatchDelta {
    let mut delta = WatchDelta::default();
    for (path, entry) in next {
        match prev.get(path) {
            None => delta.added.push(path.clone()),
            Some(old) if old != entry => delta.changed.push(path.clone()),
            Some(_) => {}
        }
    }
    for path in prev.keys() {
        if !next.contains_key(path) {
            delta.removed.push(path.clone());
        }
    }
    delta.added.sort();
    delta.removed.sort();
    delta.changed.sort();
    delta
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_snapshot_for_missing_root() {
        let missing = std::env::temp_dir().join("agenthub-watch-missing-dir");
        assert!(snapshot(&missing).is_empty());
    }

    #[test]
    fn own_artifacts_are_not_external_changes() {
        // 数据根被用户设成受管根的一部分时，索引库的写入不能进快照
        for name in ["index.db", "index.db-wal", "index.db-shm", "plan.tmp"] {
            assert!(is_own_artifact(name), "{name} 应被当成自身产物");
        }
        for name in ["SKILL.md", "cloudflare", "notes.md"] {
            assert!(!is_own_artifact(name), "{name} 是真实内容，不能忽略");
        }
    }

    #[test]
    fn unchanged_snapshot_yields_empty_delta() {
        let dir = std::env::temp_dir().join(format!("agenthub-watch-same-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.md"), "x").unwrap();
        let snap = snapshot(&dir);
        assert!(diff(&snap, &snapshot(&dir)).is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }
}
