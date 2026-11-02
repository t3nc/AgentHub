use crate::l10n::{Msg, t};
use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::paths;

use super::Registry;
use super::archive::{self, ExtractSummary};

/// 下载并解包到缓存目录后的产物。
///
/// 缓存目录位于 `~/.agenthub/market/cache/`，属于受管根之一，
/// 所以后续 `MoveIntoStore` 能正常过 `ensure_within` 门禁。
#[derive(Debug, Clone)]
pub struct PreparedSkill {
    pub name: String,
    pub dir: PathBuf,
    pub summary: ExtractSummary,
    pub version: Option<String>,
    /// 走了逐文件降级（ZIP 不可用时）
    pub fallback_files: bool,
}

/// 下载 → 解包到缓存目录。**不落任何用户可见目录**，真正的落盘交给 apply。
pub fn prepare(
    registry: &dyn Registry,
    slug: &str,
    local_name: &str,
    version: Option<&str>,
) -> Result<PreparedSkill> {
    let cache_root = paths::market_cache_root();
    paths::ensure_dir(&cache_root)?;
    purge_stale(&cache_root, local_name);

    let dir = cache_root.join(format!("{local_name}-{}", paths::unique_id()));

    let prepared = match registry.archive(slug, version) {
        Ok(bytes) => {
            let summary = archive::extract_zip(&bytes, &dir)?;
            PreparedSkill {
                name: local_name.to_string(),
                dir: dir.clone(),
                summary,
                version: version.map(str::to_string),
                fallback_files: false,
            }
        }
        Err(crate::error::Error::Unsupported(_)) => {
            let summary = fetch_files(registry, slug, local_name, version, &dir)?;
            PreparedSkill {
                name: local_name.to_string(),
                dir: dir.clone(),
                summary,
                version: version.map(str::to_string),
                fallback_files: true,
            }
        }
        Err(err) => return Err(err),
    };

    Ok(prepared)
}

/// 逐文件降级：先拿文件清单，再一个个读文本。
///
/// 二进制文件（图片等）在接口层就是 415，这里按「跳过 + 记录」处理，
/// 让用户看到「有 N 个文件没装上」，而不是整个安装失败。
fn fetch_files(
    registry: &dyn Registry,
    slug: &str,
    local_name: &str,
    version: Option<&str>,
    dest: &Path,
) -> Result<ExtractSummary> {
    let files = registry.files(slug, version).map_err(|err| {
        // 有的技能（如 GitHub 托管类）既不给 ZIP 也不给文件清单，只在源站可读
        crate::error::Error::Other(t!(Msg::ErrMarketFileList, name = local_name, error = err))
    })?;
    if files.is_empty() {
        return Err(crate::error::Error::Other(t!(
            Msg::ErrMarketNoFiles,
            name = local_name
        )));
    }

    paths::ensure_dir(dest)?;
    let mut summary = ExtractSummary::default();
    for file in &files {
        match registry.file_text(slug, &file.path, version) {
            Ok(text) => {
                archive::write_text_file(dest, &file.path, &text)?;
                summary.files += 1;
                summary.bytes = summary.bytes.saturating_add(text.len() as u64);
            }
            Err(err) => {
                summary
                    .skipped
                    .push(format!("{}（{}）", file.path, short_reason(&err)));
            }
        }
    }

    if !dest.join("SKILL.md").is_file() {
        return Err(crate::error::Error::Other(t!(Msg::ErrZipNoSkillMd)));
    }

    Ok(summary)
}

fn short_reason(err: &crate::error::Error) -> String {
    match err {
        crate::error::Error::Other(text) | crate::error::Error::Unsupported(text) => text.clone(),
        other => other.to_string(),
    }
}

/// 清掉同一技能的历史缓存目录：安装失败/取消时不至于越攒越多。
fn purge_stale(cache_root: &Path, local_name: &str) {
    let prefix = format!("{local_name}-");
    let Ok(read) = std::fs::read_dir(cache_root) else {
        return;
    };
    for entry in read.filter_map(|e| e.ok()) {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with(&prefix) {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

/// 计划被取消 / 应用成功后清理缓存目录（失败也不影响主流程）。
pub fn discard(dir: &Path) {
    if dir.exists() {
        let _ = std::fs::remove_dir_all(dir);
    }
}
