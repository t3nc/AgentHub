use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::link::copy_tree;
use crate::paths;

/// 一次 apply 的可回滚记录。manifest.json 是回滚的唯一事实来源。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ManifestEntry {
    CreatedLink {
        at: PathBuf,
        target: PathBuf,
    },
    /// 目录/文件被移动（MoveIntoStore 或 ReplaceWithLink 的原件）
    Moved {
        from: PathBuf,
        to: PathBuf,
    },
    /// 被删除（进回收站或备份）
    Removed {
        at: PathBuf,
        backup: Option<PathBuf>,
        trashed: bool,
        /// 被删的是链接时记下原指向，回滚时按平台默认策略重建。
        /// 老批次没有这个字段（`serde(default)`），反序列化不受影响。
        #[serde(default)]
        link_target: Option<PathBuf>,
    },
    Written {
        at: PathBuf,
        backup: Option<PathBuf>,
        previous_existed: bool,
    },
}

pub struct BackupSession {
    pub dir: PathBuf,
    entries: Vec<ManifestEntry>,
    seq: usize,
}

impl BackupSession {
    pub fn create() -> Result<Self> {
        let dir =
            paths::backups_root().join(format!("{}-{}", paths::ts_label(), paths::unique_id()));
        paths::ensure_dir(&dir)?;
        Ok(Self {
            dir,
            entries: Vec::new(),
            seq: 0,
        })
    }

    /// 把可能被改写的原件复制进备份目录（跨目录 rename 不可靠，统一 copy）。
    pub fn stash(&mut self, src: &Path) -> Result<Option<PathBuf>> {
        if !src.exists() && std::fs::symlink_metadata(src).is_err() {
            return Ok(None);
        }
        self.seq += 1;
        let name = src
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "item".into());
        let dest = self
            .dir
            .join("files")
            .join(format!("{}-{}", self.seq, name));
        paths::ensure_dir(&dest)?;
        if std::fs::symlink_metadata(src)
            .map(|m| m.is_file())
            .unwrap_or(false)
        {
            let target = self
                .dir
                .join("files")
                .join(format!("{}-{}.orig", self.seq, name));
            std::fs::copy(src, &target).map_err(|e| Error::io(src, e))?;
            return Ok(Some(target));
        }
        copy_tree(src, &dest)?;
        Ok(Some(dest))
    }

    pub fn record(&mut self, entry: ManifestEntry) {
        self.entries.push(entry);
    }

    pub fn entries(&self) -> &[ManifestEntry] {
        &self.entries
    }

    pub fn save(&self) -> Result<()> {
        let text = serde_json::to_string_pretty(&self.entries)?;
        paths::write_atomic(&self.dir.join("manifest.json"), text.as_bytes())
    }
}

pub fn load_manifest(dir: &Path) -> Result<Vec<ManifestEntry>> {
    let path = dir.join("manifest.json");
    let text = std::fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
    // 回滚依赖这份 manifest，别因为用户用编辑器看过一眼（加了 BOM）就整体失败
    Ok(serde_json::from_str(crate::text::strip_bom(&text))?)
}

/// 只保留最近 `keep` 个备份批次。
pub fn prune(keep: usize) {
    let root = paths::backups_root();
    let Ok(read) = std::fs::read_dir(&root) else {
        return;
    };
    let mut dirs: Vec<PathBuf> = read
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    if dirs.len() <= keep {
        return;
    }
    dirs.sort();
    let drop_count = dirs.len() - keep;
    for dir in dirs.into_iter().take(drop_count) {
        let _ = std::fs::remove_dir_all(dir);
    }
}
