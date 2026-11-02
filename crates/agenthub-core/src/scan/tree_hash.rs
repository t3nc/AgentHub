use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// 快速指纹：一次遍历拿到总大小、最新 mtime、文件数，用于判断是否需要重算哈希。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Fingerprint {
    pub size: u64,
    pub mtime: i64,
    pub files: usize,
}

pub fn fingerprint(root: &Path) -> Fingerprint {
    let real = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let mut fp = Fingerprint::default();
    for entry in walkdir::WalkDir::new(&real)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            fp.files += 1;
            if let Ok(meta) = entry.metadata() {
                fp.size += meta.len();
                if let Ok(modified) = meta.modified() {
                    let ts = modified
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs() as i64)
                        .unwrap_or(0);
                    fp.mtime = fp.mtime.max(ts);
                }
            }
        }
    }
    fp
}

/// 内容树哈希：按相对路径排序后，把 (相对路径, 大小, 内容) 依次喂给 blake3。
/// 路径与顺序都参与哈希，重命名文件也会改变结果。
pub fn tree_hash(root: &Path) -> Result<String> {
    let real = fs::canonicalize(root).map_err(|e| Error::io(root, e))?;
    let mut items: Vec<(String, PathBuf, bool, u64)> = Vec::new();
    for entry in walkdir::WalkDir::new(&real)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let rel = match entry.path().strip_prefix(&real) {
            Ok(r) if !r.as_os_str().is_empty() => r.to_string_lossy().replace('\\', "/"),
            _ => continue,
        };
        let is_dir = entry.file_type().is_dir();
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        items.push((rel, entry.path().to_path_buf(), is_dir, size));
    }
    items.sort_by(|a, b| a.0.cmp(&b.0));

    let mut hasher = blake3::Hasher::new();
    for (rel, path, is_dir, size) in items {
        hasher.update(rel.as_bytes());
        hasher.update(b"\0");
        hasher.update(if is_dir { b"d" } else { b"f" });
        hasher.update(&size.to_le_bytes());
        if !is_dir {
            let mut file = fs::File::open(&path).map_err(|e| Error::io(&path, e))?;
            std::io::copy(&mut file, &mut hasher).map_err(|e| Error::io(&path, e))?;
        }
    }
    Ok(hasher.finalize().to_hex().to_string())
}

/// 判断两份内容是否一致（哈希可用时直接比哈希）。
pub fn same_content(a: &Path, b: &Path) -> bool {
    match (tree_hash(a), tree_hash(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}
