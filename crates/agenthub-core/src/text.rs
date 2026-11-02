//! 换行符保真。
//!
//! Windows 上用户的 `AGENTS.md` / `CLAUDE.md` / `config.yaml` 多为 CRLF。写入层如果
//! 一律按 LF 拼行，用户看到的是一整文件被重写（每行都变），而不是「只动了受控区块」。
//!
//! 约定：
//! - 读入时用 [`normalize_lf`] 归一化，业务逻辑一律只处理 `\n`；
//! - 写回前用 [`with_eol`] 还原成原文件的 EOL；
//! - 新建文件用 LF（与仓库 `.gitattributes` 一致，agent 工具链普遍接受）。
//!
//! 预览与实际落盘保持同一份字节：EOL 在生成层还原，写入层不做隐式转换。
//!
//! 另外处理 Windows 常见的 UTF-8 BOM：记事本、PowerShell 5.1 的 `Set-Content`、
//! Visual Studio 默认都会写 BOM，而 `serde_json` / `toml_edit` 会把它当非法字符。
//! 解析前用 [`strip_bom`] 去掉，需要保真时再用 [`with_bom_if`] 还原。

const BOM: char = '\u{feff}';

/// 去掉开头的 UTF-8 BOM。
pub fn strip_bom(text: &str) -> &str {
    text.strip_prefix(BOM).unwrap_or(text)
}

/// 原文本是否带 BOM。
pub fn has_bom(text: &str) -> bool {
    text.starts_with(BOM)
}

/// 原文本带 BOM 时，把 BOM 补回输出，保证「逐字节保留」的语义。
pub fn with_bom_if(had_bom: bool, text: String) -> String {
    if had_bom && !text.starts_with(BOM) {
        format!("{BOM}{text}")
    } else {
        text
    }
}

/// 探测行尾风格：以第一个换行为准；没有换行或缺省时按 LF。
pub fn eol_of(text: &str) -> &'static str {
    match text.as_bytes().iter().position(|b| *b == b'\n') {
        Some(idx) if idx > 0 && text.as_bytes()[idx - 1] == b'\r' => "\r\n",
        _ => "\n",
    }
}

/// 是否是 CRLF 文本。
pub fn is_crlf(text: &str) -> bool {
    eol_of(text) == "\r\n"
}

/// 归一化成 LF（同时把孤立的 `\r` 也当换行），用于比较与内部处理。
pub fn normalize_lf(text: &str) -> String {
    if !text.contains('\r') {
        return text.to_string();
    }
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// 把文本的行尾统一成 `eol`，末尾是否有换行保持不变。
pub fn with_eol(text: &str, eol: &str) -> String {
    let lf = normalize_lf(text);
    if eol == "\n" {
        lf
    } else {
        lf.replace('\n', eol)
    }
}

/// 按指定 EOL 拼接多行。
pub fn join_lines<'a, I>(lines: I, eol: &str) -> String
where
    I: IntoIterator<Item = &'a str>,
{
    lines.into_iter().collect::<Vec<_>>().join(eol)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_eol() {
        assert_eq!(eol_of("a\r\nb"), "\r\n");
        assert_eq!(eol_of("a\nb"), "\n");
        assert_eq!(eol_of("no-newline"), "\n");
        assert_eq!(eol_of(""), "\n");
    }

    #[test]
    fn round_trips_crlf() {
        let crlf = "# t\r\n\r\nbody\r\n";
        let restored = with_eol(&normalize_lf(crlf), eol_of(crlf));
        assert_eq!(restored, crlf);
    }

    #[test]
    fn normalizes_lone_cr() {
        assert_eq!(normalize_lf("a\rb"), "a\nb");
    }

    #[test]
    fn keeps_missing_trailing_newline() {
        assert_eq!(with_eol("a\nb", "\r\n"), "a\r\nb");
    }
}
