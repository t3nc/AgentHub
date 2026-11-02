//! 终端会话索引：`index.json` 的读写、保留策略与文件名清洗。
//!
//! 隐私前提：「终端输出可能包含密钥」，所以这里也负责给出上限——
//! 单文件 2MB、最多 20 个会话、超过 7 天的直接淘汰，避免日志无限增长。

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{error::Result, paths};

/// 最多保留这么多个会话
pub const KEEP_SESSIONS: usize = 20;
/// 再老就删掉（天）
pub const KEEP_DAYS: i64 = 7;
/// 单个会话日志上限，超出后写线程停止追加
pub const MAX_LOG_BYTES: u64 = 2 * 1024 * 1024;

/// 一个终端标签的元数据。进程不恢复，只恢复「上次在这干了什么」。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionMeta {
    pub id: String,
    pub title: String,
    /// 起 shell 后自动敲的那行（agent CLI），纯 shell 为 None
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub cwd: String,
    /// 标签对应的 agent 图标 id
    #[serde(default)]
    pub agent: Option<String>,
    /// unix 秒
    pub started_at: i64,
    /// starting / running / exited / error
    #[serde(default)]
    pub status: String,
    /// 已写入日志的字节数
    #[serde(default)]
    pub bytes: u64,
    /// 是否因超过 MAX_LOG_BYTES 被截断
    #[serde(default)]
    pub truncated: bool,
}

pub fn dir() -> PathBuf {
    paths::terminal_logs_root()
}

pub fn index_path() -> PathBuf {
    dir().join("index.json")
}

/// 日志文件名：id 由前端生成（`term-<uuid>`），仍然清洗一遍防路径穿越。
pub fn log_path(id: &str) -> PathBuf {
    dir().join(format!("{}.log", sanitize(id)))
}

/// 文件名清洗：保留字母数字与 `-_.`，其余换 `-`，并截断到 80 字符。
pub fn sanitize(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|ch| {
            if ch.is_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
                ch
            } else {
                '-'
            }
        })
        .take(80)
        .collect();
    let trimmed = cleaned.trim_matches('-');
    // 纯点/横线的结果（".."、"..."）拼成文件名会变成隐藏文件或上级目录，一律弃用
    if trimmed.is_empty()
        || trimmed
            .chars()
            .all(|ch| ch == '.' || ch == '-' || ch == '_')
    {
        "session".to_string()
    } else {
        trimmed.to_string()
    }
}

/// 读索引。文件缺失或内容坏掉一律当作空列表：历史丢了不该影响开终端。
pub fn load() -> Vec<SessionMeta> {
    let Ok(bytes) = std::fs::read(index_path()) else {
        return Vec::new();
    };
    serde_json::from_slice(&bytes).unwrap_or_default()
}

pub fn save(entries: &[SessionMeta]) -> Result<()> {
    let json = serde_json::to_vec_pretty(entries)?;
    paths::write_atomic(&index_path(), &json)
}

/// 插入或更新（同 id 覆盖），保持按开始时间倒序。
pub fn upsert(entries: &mut Vec<SessionMeta>, meta: SessionMeta) {
    match entries.iter_mut().find(|entry| entry.id == meta.id) {
        Some(slot) => *slot = meta,
        None => entries.push(meta),
    }
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.started_at));
}

/// 保留策略：最近 `KEEP_SESSIONS` 个、且不超过 `KEEP_DAYS` 天。
///
/// 返回 (保留项, 被淘汰的 id)。输入顺序无关，内部先按时间倒序。
pub fn prune(entries: Vec<SessionMeta>, now: i64) -> (Vec<SessionMeta>, Vec<String>) {
    let cutoff = now - KEEP_DAYS * 86_400;
    let mut sorted = entries;
    sorted.sort_by_key(|entry| std::cmp::Reverse(entry.started_at));

    let mut kept = Vec::new();
    let mut dropped = Vec::new();
    for (index, entry) in sorted.into_iter().enumerate() {
        if index < KEEP_SESSIONS && entry.started_at >= cutoff {
            kept.push(entry);
        } else {
            dropped.push(entry.id);
        }
    }
    (kept, dropped)
}

/// 删除某个会话的日志文件（索引条目由调用方处理）。
pub fn remove_log(id: &str) {
    let _ = std::fs::remove_file(log_path(id));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(id: &str, started_at: i64) -> SessionMeta {
        SessionMeta {
            id: id.to_string(),
            title: id.to_string(),
            command: None,
            cwd: "/tmp".to_string(),
            agent: None,
            started_at,
            status: "exited".to_string(),
            bytes: 0,
            truncated: false,
        }
    }

    #[test]
    fn sanitize_strips_path_separators() {
        assert_eq!(sanitize("term-1/../etc"), "term-1-..-etc");
        assert_eq!(sanitize("../.."), "session");
        assert_eq!(sanitize(""), "session");
        assert_eq!(sanitize("终端 标签"), "终端-标签");
    }

    #[test]
    fn upsert_replaces_and_sorts() {
        let mut entries = vec![meta("a", 100), meta("b", 300)];
        upsert(&mut entries, meta("a", 200));
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].id, "b");
        assert_eq!(entries[1].started_at, 200);
    }

    #[test]
    fn prune_keeps_newest_and_drops_expired() {
        let now = 1_000_000;
        let mut entries: Vec<SessionMeta> = (0..25)
            .map(|index| meta(&format!("s{index}"), now - index as i64 * 60))
            .collect();
        // 一条 8 天前的老记录
        entries.push(meta("old", now - 8 * 86_400));

        let (kept, dropped) = prune(entries, now);
        assert_eq!(kept.len(), KEEP_SESSIONS);
        assert!(dropped.contains(&"old".to_string()));
        assert_eq!(kept[0].id, "s0", "最新的一条必须留");
        assert!(kept.iter().all(|entry| entry.id != "old"));
        assert_eq!(dropped.len(), 25 + 1 - KEEP_SESSIONS);
    }

    #[test]
    fn prune_is_order_independent() {
        let now = 1_000_000;
        let entries = vec![meta("b", now - 10), meta("a", now - 20)];
        let (kept, dropped) = prune(entries, now);
        assert!(dropped.is_empty());
        assert_eq!(kept[0].id, "b");
    }
}
