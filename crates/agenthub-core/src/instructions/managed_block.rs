use crate::error::{Error, Result};
use crate::l10n::{Msg, t};
use crate::model::instruction::{BLOCK_BEGIN, BLOCK_END, MAX_INSTRUCTION_BYTES, ManagedBlockInfo};
use crate::text;

/// 定位受控区块（含起止标记行，索引从 1 开始）。
pub fn find_block(text: &str) -> ManagedBlockInfo {
    let eol = text::eol_of(text);
    let lines: Vec<&str> = text.lines().collect();
    let begin = lines.iter().position(|l| l.trim() == BLOCK_BEGIN);
    let end = lines.iter().position(|l| l.trim() == BLOCK_END);
    match (begin, end) {
        (Some(b), Some(e)) if e > b => {
            let body = text::join_lines(lines[b + 1..e].iter().copied(), eol);
            ManagedBlockInfo {
                present: true,
                start_line: Some(b + 1),
                end_line: Some(e + 1),
                body: Some(body),
            }
        }
        _ => ManagedBlockInfo {
            present: false,
            start_line: None,
            end_line: None,
            body: None,
        },
    }
}

/// 注入或更新受控区块；用户自有内容（区块之外）逐字节保留，行尾跟随原文件。
pub fn inject(existing: Option<&str>, body: &str) -> Result<String> {
    // 有原文件就跟随它的 EOL；新建文件用 LF
    let eol = existing.map(text::eol_of).unwrap_or("\n");
    let normalized = existing.map(text::normalize_lf);
    let out = inject_lf(normalized.as_deref(), body)?;
    Ok(text::with_eol(&out, eol))
}

fn inject_lf(existing: Option<&str>, body: &str) -> Result<String> {
    if body.len() > MAX_INSTRUCTION_BYTES {
        return Err(Error::Other(t!(
            Msg::ErrManagedBlockTooLarge,
            limit = MAX_INSTRUCTION_BYTES
        )));
    }
    let block = format!("{BLOCK_BEGIN}\n{}\n{BLOCK_END}", body.trim_end());

    let Some(text) = existing else {
        return Ok(format!("{block}\n"));
    };
    if text.len() > MAX_INSTRUCTION_BYTES {
        return Err(Error::Other(t!(
            Msg::ErrInstructionTooLarge,
            path = t!(Msg::LabelInstructionFile),
            limit = MAX_INSTRUCTION_BYTES
        )));
    }

    let info = find_block(text);
    if info.present {
        let lines: Vec<&str> = text.lines().collect();
        let start = info.start_line.unwrap_or(1) - 1;
        let end = info.end_line.unwrap_or(lines.len());
        let mut out: Vec<String> = Vec::new();
        out.extend(lines[..start].iter().map(|s| s.to_string()));
        out.extend(block.lines().map(|s| s.to_string()));
        out.extend(lines[end..].iter().map(|s| s.to_string()));
        let mut joined = out.join("\n");
        if text.ends_with('\n') || !joined.ends_with('\n') {
            joined.push('\n');
        }
        return Ok(joined);
    }

    let mut out = text.to_string();
    if !out.ends_with('\n') {
        out.push('\n');
    }
    if !out.trim().is_empty() {
        out.push('\n');
    }
    out.push_str(&block);
    out.push('\n');
    Ok(out)
}

/// 移除受控区块（卸载 / 清理时用），行尾跟随原文件。
pub fn strip(text: &str) -> String {
    let eol = text::eol_of(text);
    let normalized = text::normalize_lf(text);
    let out = strip_lf(&normalized);
    text::with_eol(&out, eol)
}

fn strip_lf(text: &str) -> String {
    let info = find_block(text);
    if !info.present {
        return text.to_string();
    }
    let lines: Vec<&str> = text.lines().collect();
    let start = info.start_line.unwrap_or(1) - 1;
    let end = info.end_line.unwrap_or(lines.len());
    let mut out: Vec<&str> = Vec::new();
    out.extend_from_slice(&lines[..start]);
    out.extend_from_slice(&lines[end..]);
    out.join("\n")
}

/// 写入前复核：内容哈希一致才允许覆盖，防止外部工具在 plan→apply 之间改文件。
///
/// 比较前先做 EOL 归一化：外部工具只改了行尾（CRLF↔LF）不该被当成「内容被改」，
/// 否则 Windows 上会频繁误报。代价是「仅改行尾」不再被检测到。
pub fn verify_unchanged(expected: &str, actual: Option<&str>) -> Result<()> {
    let actual = actual.unwrap_or("");
    if hash(&text::normalize_lf(expected)) == hash(&text::normalize_lf(actual)) {
        return Ok(());
    }
    Err(Error::Conflict(t!(Msg::ErrFileChangedSincePlan)))
}

fn hash(text: &str) -> String {
    blake3::hash(text.as_bytes()).to_hex().to_string()
}
