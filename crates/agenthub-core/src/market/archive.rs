use crate::l10n::{Msg, t};
use std::io::{Cursor, Read};
use std::path::{Component, Path, PathBuf};

use crate::error::{Error, Result};
use crate::paths;

use super::{MAX_ARCHIVE_BYTES, MAX_ARCHIVE_FILES};

/// 解包结果：给计划里的 note 用。
#[derive(Debug, Clone, Default)]
pub struct ExtractSummary {
    pub files: usize,
    pub bytes: u64,
    /// 被跳过的条目（符号链接、目录项等），用于提示而不是静默丢弃
    pub skipped: Vec<String>,
}

/// 把技能包 ZIP 解到 `dest`（必须已存在或由本函数创建）。
///
/// 安全约束（任一不满足直接失败，**不做部分成功**）：
/// 1. 只接受 `Component::Normal` 组成的相对路径 —— 绝对路径、盘符、`..` 全部拒绝；
/// 2. 符号链接条目一律跳过（包里的链接可以指向任意位置）；
/// 3. 文件数与解压后总字节数都有上限，防 zip bomb；
/// 4. 根部必须有 `SKILL.md`，否则不算技能包。
pub fn extract_zip(bytes: &[u8], dest: &Path) -> Result<ExtractSummary> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|err| Error::Other(t!(Msg::ErrZipInvalid, error = err)))?;

    if archive.len() > MAX_ARCHIVE_FILES {
        return Err(Error::Other(t!(
            Msg::ErrZipTooManyEntries,
            count = archive.len(),
            limit = MAX_ARCHIVE_FILES
        )));
    }

    paths::ensure_dir(dest)?;
    let mut summary = ExtractSummary::default();
    let mut total: u64 = 0;

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|err| Error::Other(t!(Msg::ErrZipReadEntry, error = err)))?;

        let raw_name = match entry.name() {
            Ok(name) => name.to_string(),
            Err(err) => {
                summary
                    .skipped
                    .push(t!(Msg::ZipSkipUnreadableName, error = err));
                continue;
            }
        };
        let Some(rel) = sanitize_relative(&raw_name) else {
            summary
                .skipped
                .push(t!(Msg::ZipSkipEscaped, name = raw_name));
            continue;
        };

        // 符号链接：包里的软链可能指向系统任意位置，一律不落地
        if is_symlink_entry(entry.unix_mode()) {
            summary
                .skipped
                .push(t!(Msg::ZipSkipSymlink, path = rel.display()));
            continue;
        }

        let target = dest.join(&rel);
        if entry.is_dir() {
            paths::ensure_dir(&target)?;
            continue;
        }

        let declared = entry.size();
        if total.saturating_add(declared) > MAX_ARCHIVE_BYTES {
            return Err(Error::Other(t!(
                Msg::ErrZipTooLargeExtract,
                limit = crate::api::human_bytes(MAX_ARCHIVE_BYTES)
            )));
        }

        let mut buffer = Vec::with_capacity(declared.min(1024 * 1024) as usize);
        entry.read_to_end(&mut buffer).map_err(|err| {
            Error::Other(t!(
                Msg::ErrZipExtractFailed,
                path = rel.display(),
                error = err
            ))
        })?;
        // 只信实际读到的字节数：声明值可以被伪造成很小
        total = total.saturating_add(buffer.len() as u64);
        if total > MAX_ARCHIVE_BYTES {
            return Err(Error::Other(t!(Msg::ErrZipTooLarge)));
        }

        if let Some(parent) = target.parent() {
            paths::ensure_dir(parent)?;
        }
        std::fs::write(crate::platform::long_path(&target), &buffer)
            .map_err(|err| Error::io(&target, err))?;
        summary.files += 1;
    }

    summary.bytes = total;

    if !dest.join("SKILL.md").is_file() {
        return Err(Error::Other(t!(Msg::ErrZipNoSkillMd)));
    }

    Ok(summary)
}

/// 逐文件落盘（ZIP 下载不可用时的降级路径）。
pub fn write_text_file(dest: &Path, rel: &str, content: &str) -> Result<()> {
    let rel_path = sanitize_relative(rel)
        .ok_or_else(|| Error::UnsafePath(t!(Msg::ErrZipPathEscape, path = rel)))?;
    let target = dest.join(rel_path);
    if let Some(parent) = target.parent() {
        paths::ensure_dir(parent)?;
    }
    std::fs::write(crate::platform::long_path(&target), content.as_bytes())
        .map_err(|err| Error::io(&target, err))
}

fn sanitize_relative(raw: &str) -> Option<PathBuf> {
    if raw.is_empty() || raw.contains('\0') {
        return None;
    }
    // ZIP 规范用 `/`；Windows 侧可能混入 `\`，统一按分隔符切
    let mut out = PathBuf::new();
    for part in raw.split(['/', '\\']) {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return None;
        }
        // 盘符或 UNC 前缀
        if part.len() >= 2 && part.as_bytes()[1] == b':' {
            return None;
        }
        out.push(part);
    }
    if out.as_os_str().is_empty() {
        return None;
    }
    // 双保险：结果必须全是普通组件
    if out.components().any(|c| !matches!(c, Component::Normal(_))) {
        return None;
    }
    Some(out)
}

fn is_symlink_entry(mode: Option<u32>) -> bool {
    matches!(mode, Some(m) if m & 0o170000 == 0o120000)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    fn build_zip(entries: &[(&str, &str)]) -> Vec<u8> {
        let mut buffer = Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut buffer);
            let options =
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
            for (name, body) in entries {
                writer.start_file(*name, options).unwrap();
                writer.write_all(body.as_bytes()).unwrap();
            }
            writer.finish().unwrap();
        }
        buffer.into_inner()
    }

    #[test]
    fn extracts_normal_skill() {
        let bytes = build_zip(&[
            ("SKILL.md", "---\nname: demo\n---\nbody"),
            ("references/a.md", "hello"),
        ]);
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("demo");
        let summary = extract_zip(&bytes, &dest).unwrap();
        assert_eq!(summary.files, 2);
        assert!(summary.bytes > 0);
        assert!(dest.join("SKILL.md").is_file());
        assert!(dest.join("references/a.md").is_file());
    }

    #[test]
    fn rejects_path_traversal() {
        let bytes = build_zip(&[("SKILL.md", "x"), ("../evil.txt", "boom")]);
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("demo");
        let summary = extract_zip(&bytes, &dest).unwrap();
        // 越界条目被跳过，且没有写到 dest 之外
        assert_eq!(summary.skipped.len(), 1);
        assert!(!dir.path().join("evil.txt").exists());
        assert!(dest.join("SKILL.md").is_file());
    }

    #[test]
    fn rejects_missing_skill_md() {
        let bytes = build_zip(&[("README.md", "no skill here")]);
        let dir = tempfile::tempdir().unwrap();
        let err = extract_zip(&bytes, &dir.path().join("demo")).unwrap_err();
        assert!(err.to_string().contains("SKILL.md"), "{err}");
    }

    #[test]
    fn rejects_too_many_entries() {
        let names: Vec<String> = (0..(MAX_ARCHIVE_FILES + 5))
            .map(|i| format!("f{i}.md"))
            .collect();
        let entries: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), "x")).collect();
        let bytes = build_zip(&entries);
        let dir = tempfile::tempdir().unwrap();
        let err = extract_zip(&bytes, &dir.path().join("demo")).unwrap_err();
        assert!(err.to_string().contains("上限"), "{err}");
    }

    #[test]
    fn write_text_file_blocks_escape() {
        let dir = tempfile::tempdir().unwrap();
        assert!(write_text_file(dir.path(), "../x.md", "a").is_err());
        assert!(write_text_file(dir.path(), "C:/x.md", "a").is_err());
        assert!(write_text_file(dir.path(), "a/b.md", "a").is_ok());
    }
}
