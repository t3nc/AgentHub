use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::l10n::{self, Msg, t};
use crate::model::skill::LinkState;
use crate::paths;
use crate::platform;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkStrategy {
    /// Unix 符号链接 / Windows 目录符号链接
    Symlink,
    /// Windows 目录联接（免管理员权限）
    Junction,
    /// 目录内逐文件硬链接（同卷、省空间、对不识别链接的程序更安全）
    Hardlink,
    /// 完整复制（兼容性兜底）
    Copy,
}

impl LinkStrategy {
    /// 平台默认：mac/Linux 软链，Windows 目录联接。
    pub fn platform_default() -> Self {
        if cfg!(windows) {
            LinkStrategy::Junction
        } else {
            LinkStrategy::Symlink
        }
    }

    pub fn is_supported(self) -> bool {
        match self {
            LinkStrategy::Symlink | LinkStrategy::Copy => true,
            LinkStrategy::Hardlink => true,
            LinkStrategy::Junction => cfg!(windows),
        }
    }

    pub fn label(self) -> &'static str {
        l10n::str_of(match self {
            LinkStrategy::Symlink => Msg::StrategySymlink,
            LinkStrategy::Junction => Msg::StrategyJunction,
            LinkStrategy::Hardlink => Msg::StrategyHardlink,
            LinkStrategy::Copy => Msg::StrategyCopy,
        })
    }

    pub fn slug(self) -> &'static str {
        match self {
            LinkStrategy::Symlink => "symlink",
            LinkStrategy::Junction => "junction",
            LinkStrategy::Hardlink => "hardlink",
            LinkStrategy::Copy => "copy",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "symlink" | "soft" => Some(LinkStrategy::Symlink),
            "junction" => Some(LinkStrategy::Junction),
            "hardlink" | "hard" => Some(LinkStrategy::Hardlink),
            "copy" => Some(LinkStrategy::Copy),
            _ => None,
        }
    }

    /// 解析顺序：单条覆盖 > 配置默认 > 平台默认。
    pub fn resolve(override_value: Option<Self>, config_default: Option<Self>) -> Self {
        override_value
            .or(config_default)
            .unwrap_or_else(LinkStrategy::platform_default)
    }
}

/// 在 `at` 位置建立指向 `target` 的链接（或复制）。
pub fn create_dir_link(target: &Path, at: &Path, strategy: LinkStrategy) -> Result<()> {
    if let Some(parent) = at.parent() {
        paths::ensure_dir(parent)?;
    }
    if at.exists() || fs::symlink_metadata(at).is_ok() {
        return Err(Error::Conflict(t!(Msg::ErrLinkExists, path = at.display())));
    }
    match strategy {
        LinkStrategy::Symlink => symlink_dir(target, at),
        LinkStrategy::Junction => junction_dir(target, at),
        LinkStrategy::Hardlink => hardlink_tree(target, at),
        LinkStrategy::Copy => copy_tree(target, at),
    }
}

#[cfg(not(windows))]
fn symlink_dir(target: &Path, at: &Path) -> Result<()> {
    std::os::unix::fs::symlink(target, platform::long_path(at)).map_err(|e| Error::io(at, e))
}

#[cfg(windows)]
fn symlink_dir(target: &Path, at: &Path) -> Result<()> {
    std::os::windows::fs::symlink_dir(target, platform::long_path(at)).map_err(|e| {
        Error::Unsupported(t!(
            Msg::ErrSymlinkUnsupported,
            path = at.display(),
            error = e
        ))
    })
}

#[cfg(windows)]
fn junction_dir(target: &Path, at: &Path) -> Result<()> {
    junction::create(platform::long_path(target), platform::long_path(at))
        .map_err(|e| Error::io(at, e))
}

#[cfg(not(windows))]
fn junction_dir(_target: &Path, at: &Path) -> Result<()> {
    Err(Error::Unsupported(t!(
        Msg::ErrJunctionNotSupported,
        path = at.display()
    )))
}

pub fn copy_tree(src: &Path, dst: &Path) -> Result<()> {
    paths::ensure_dir(dst)?;
    let long_src = platform::long_path(src);
    for entry in walkdir::WalkDir::new(&long_src).follow_links(false) {
        let entry = entry.map_err(|e| Error::msg(t!(Msg::ErrWalkFailed, error = e)))?;
        let rel = entry
            .path()
            .strip_prefix(&long_src)
            .map_err(|_| Error::msg(t!(Msg::ErrStripPrefix)))?;
        if rel.as_os_str().is_empty() {
            continue;
        }
        let out = dst.join(rel);
        if platform::is_link_like(entry.path()) {
            // 保留原链接形态，避免把链接指向的内容展开（Windows 的 junction 也走这里）
            copy_symlink_entry(entry.path(), &out)?;
        } else if entry.file_type().is_dir() {
            paths::ensure_dir(&out)?;
        } else {
            fs::copy(entry.path(), platform::long_path(&out)).map_err(|e| Error::io(&out, e))?;
        }
    }
    Ok(())
}

/// 复制一条链接：按目标的真实类型决定建目录链接还是文件链接。
///
/// 旧实现在 Windows 上固定 `symlink_file`，源是目录链接时必失败且错误被吞掉。
fn copy_symlink_entry(from: &Path, to: &Path) -> Result<()> {
    let real = fs::read_link(from).map_err(|e| Error::io(from, e))?;
    let target_is_dir = fs::metadata(from).map(|m| m.is_dir()).unwrap_or(false);
    if target_is_dir {
        if create_dir_link(&real, to, LinkStrategy::platform_default()).is_ok() {
            return Ok(());
        }
        // 目录链接建不出来（Windows 无权限时的 symlink）→ 退到目录联接/复制
        if create_dir_link(&real, to, LinkStrategy::Copy).is_ok() {
            return Ok(());
        }
    } else {
        #[cfg(unix)]
        {
            if std::os::unix::fs::symlink(&real, to).is_ok() {
                return Ok(());
            }
        }
        #[cfg(windows)]
        {
            if std::os::windows::fs::symlink_file(&real, to).is_ok() {
                return Ok(());
            }
        }
        if fs::copy(from, platform::long_path(to)).is_ok() {
            return Ok(());
        }
    }
    Err(Error::msg(t!(
        Msg::ErrCopyLink,
        from = from.display(),
        to = to.display()
    )))
}

pub fn hardlink_tree(src: &Path, dst: &Path) -> Result<()> {
    paths::ensure_dir(dst)?;
    let long_src = platform::long_path(src);
    for entry in walkdir::WalkDir::new(&long_src).follow_links(false) {
        let entry = entry.map_err(|e| Error::msg(t!(Msg::ErrWalkFailed, error = e)))?;
        let rel = entry
            .path()
            .strip_prefix(&long_src)
            .map_err(|_| Error::msg(t!(Msg::ErrStripPrefix)))?;
        if rel.as_os_str().is_empty() {
            continue;
        }
        let out = dst.join(rel);
        if entry.file_type().is_dir() {
            paths::ensure_dir(&out)?;
        } else if entry.file_type().is_file()
            && fs::hard_link(entry.path(), platform::long_path(&out)).is_err()
        {
            fs::copy(entry.path(), platform::long_path(&out)).map_err(|e| Error::io(&out, e))?;
        }
    }
    Ok(())
}

/// 判断某个技能目录当前的链接状态。`store_root` 用于区分「受管链接」与「外部链接」。
pub fn link_state(store_root: &Path, path: &Path) -> LinkState {
    let meta = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(_) => return LinkState::RealDir,
    };

    if meta.file_type().is_symlink() || platform::is_junction(path) {
        let target = fs::read_link(platform::long_path(path)).unwrap_or_default();
        let resolved = resolve_link_target(path, &target);
        if !resolved.exists() {
            return LinkState::Broken { target: resolved };
        }
        if paths::is_within(store_root, &resolved) {
            return if platform::is_junction(path) {
                LinkState::Junction { target: resolved }
            } else {
                LinkState::Symlink { target: resolved }
            };
        }
        return LinkState::Foreign { target: resolved };
    }

    // 硬链接：与 store 中同名目录内文件 inode 相同即视为受管
    if let Some(target) = detect_hardlinked_copy(store_root, path) {
        return LinkState::Hardlink { target };
    }

    LinkState::RealDir
}

/// 相对链接按链接自身所在目录解析。
fn resolve_link_target(link: &Path, target: &Path) -> PathBuf {
    if target.is_absolute() {
        return paths::normalize_lexical(target);
    }
    match link.parent() {
        Some(parent) => paths::normalize_lexical(&parent.join(target)),
        None => paths::normalize_lexical(target),
    }
}

/// 硬链接副本探测：与 store 中同名目录内的同名文件比较文件身份。
///
/// Windows 上「文件身份」= 卷序列号 + 文件索引，由 [`platform::same_file`] 统一处理，
/// 不再像旧实现那样在非 unix 平台直接返回 None。
fn detect_hardlinked_copy(store_root: &Path, path: &Path) -> Option<PathBuf> {
    let name = path.file_name()?;
    let candidate = store_root.join(name);
    if !candidate.is_dir() {
        return None;
    }
    let probe = fs::read_dir(platform::long_path(path))
        .ok()?
        .filter_map(|e| e.ok())
        .find(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))?;
    let other = candidate.join(probe.file_name());
    if platform::same_file(&probe.path(), &other) {
        Some(candidate)
    } else {
        None
    }
}

/// 删除一个链接或目录。
///
/// 链接（含 Windows junction）只摘掉链接本身；实体目录进回收站或整目录删除。
/// 这是 Windows 上最危险的一条路径：把 junction 当实体目录 `remove_dir_all`
/// 会沿 reparse point 递归删掉 store 里的真身。
pub fn remove_linked_or_dir(path: &Path, use_trash: bool) -> Result<()> {
    if fs::symlink_metadata(platform::long_path(path)).is_err() {
        return Ok(());
    }
    if platform::is_link_like(path) {
        return platform::remove_link(path);
    }
    if use_trash {
        trash::delete(path)
            .map_err(|e| Error::msg(t!(Msg::ErrTrashFailed, path = path.display(), error = e)))?;
        return Ok(());
    }
    fs::remove_dir_all(platform::long_path(path)).map_err(|e| Error::io(path, e))
}
