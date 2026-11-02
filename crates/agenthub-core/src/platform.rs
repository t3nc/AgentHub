//! 平台差异的唯一入口。
//!
//! 业务层只调用这里的语义 API（「这是不是链接」「删掉这个链接」），不直接写 `cfg`；
//! 每个函数都同时提供 Windows 与 Unix 实现，保证两平台编译都不缺分支。
//!
//! 关键语义：Windows 的目录联接（junction）在 `file_type().is_symlink()` 上返回
//! `false`，但遍历时会被当普通目录递归——所以删除/搬移前必须用 [`is_link_like`]
//! 判定，并只用 [`remove_link`] 摘链接本身，绝不能落到 `remove_dir_all`。

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// 是否是链接（Unix 符号链接 / Windows 符号链接或目录联接）。
pub fn is_link_like(path: &Path) -> bool {
    match fs::symlink_metadata(long_path(path)) {
        Ok(meta) => meta.file_type().is_symlink() || is_junction(path),
        Err(_) => false,
    }
}

/// Windows 目录联接判定；其他平台恒为 false。
#[cfg(windows)]
pub fn is_junction(path: &Path) -> bool {
    junction::exists(long_path(path)).unwrap_or(false)
}

#[cfg(not(windows))]
pub fn is_junction(_path: &Path) -> bool {
    false
}

/// 删除链接本身。
///
/// Windows 上 `remove_dir` 只摘掉 reparse point，不会递归进目标；Unix 上 `remove_file`
/// 同理。任何情况下都不会触碰链接指向的内容。
pub fn remove_link(path: &Path) -> Result<()> {
    let at = long_path(path);
    #[cfg(windows)]
    {
        fs::remove_dir(&at)
            .or_else(|_| fs::remove_file(&at))
            .map_err(|e| Error::io(path, e))
    }
    #[cfg(not(windows))]
    {
        fs::remove_file(&at).map_err(|e| Error::io(path, e))
    }
}

/// 读取链接指向的目标；不是链接或读不到时返回 None。
///
/// 回滚需要它：删链接时没有内容可备份，只能记下「原来指向哪」再重建。
#[cfg(windows)]
pub fn link_target(path: &Path) -> Option<PathBuf> {
    let at = long_path(path);
    junction::get_target(&at)
        .ok()
        .or_else(|| fs::read_link(&at).ok())
}

#[cfg(not(windows))]
pub fn link_target(path: &Path) -> Option<PathBuf> {
    fs::read_link(long_path(path)).ok()
}

/// 两个路径是否指向同一份文件内容（硬链接探测用）。
///
/// 用 `same-file`：Unix 比 `(dev, ino)`，Windows 比 `(卷序列号, 文件索引)`。
/// 不用 `std::os::windows::fs::MetadataExt::file_index`——它至今仍是 unstable
/// （`windows_by_handle`），stable 工具链编不过。
pub fn same_file(a: &Path, b: &Path) -> bool {
    same_file::is_same_file(long_path(a), long_path(b)).unwrap_or(false)
}

/// Windows 长路径：只在真正的 syscall 边界加 `\\?\` 前缀，绝不写进内存路径、
/// SQLite、Diff 或 UI。
///
/// 少于 240 字符的路径原样返回（保持普通语义）；相对路径无法加前缀，原样返回。
pub fn long_path(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        let Some(s) = path.to_str() else {
            return path.to_path_buf();
        };
        if s.len() < 240 || s.starts_with(r"\\?\") || s.starts_with(r"\\.\") {
            return path.to_path_buf();
        }
        if let Some(rest) = s.strip_prefix(r"\\") {
            return PathBuf::from(format!(r"\\?\UNC\{rest}"));
        }
        if path.is_absolute() {
            return PathBuf::from(format!(r"\\?\{s}"));
        }
        path.to_path_buf()
    }
    #[cfg(not(windows))]
    {
        path.to_path_buf()
    }
}

/// 跨平台一致地跳过的系统垃圾文件。
pub fn is_system_junk(name: &str) -> bool {
    const JUNK: [&str; 4] = ["desktop.ini", "thumbs.db", ".ds_store", ".localized"];
    JUNK.iter().any(|j| name.eq_ignore_ascii_case(j))
}

/// 展示用路径：分隔符统一成 `/`，避免 Windows 上出现 `~/foo\bar` 这种混合形态。
pub fn display(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn junk_names_are_case_insensitive() {
        assert!(is_system_junk("Desktop.ini"));
        assert!(is_system_junk(".DS_Store"));
        assert!(is_system_junk("Thumbs.db"));
        assert!(!is_system_junk("SKILL.md"));
    }

    #[test]
    fn long_path_is_stable_for_short_paths() {
        let p = Path::new("/tmp/agenthub/x");
        assert_eq!(long_path(p), p.to_path_buf());
    }
}
