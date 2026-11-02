use super::managed_block;

/// Claude Code 支持 `@relative/path` 导入语法，因此桥接只写一行引用，
/// 不复制正文——AGENTS.md 改一次，Claude 那边立即生效。
pub fn bridge_body(import_path: &str) -> String {
    format!("@{import_path}")
}

/// 判断受控区块里是否已经包含该 import。
pub fn has_import(existing: Option<&str>, import_path: &str) -> bool {
    let Some(text) = existing else {
        return false;
    };
    let info = managed_block::find_block(text);
    match info.body {
        Some(body) => body.contains(&format!("@{import_path}")),
        None => false,
    }
}

/// 生成「确保 import 存在」后的全文。
pub fn ensure_import(existing: Option<&str>, import_path: &str) -> String {
    let body = bridge_body(import_path);
    managed_block::inject(existing, &body).unwrap_or(body)
}

/// 简易的 `@path` 断链检测：返回文件中引用但磁盘上不存在的相对路径。
pub fn broken_imports(text: &str, base_dir: &std::path::Path) -> Vec<String> {
    let mut out = Vec::new();
    for token in text.split_whitespace() {
        let Some(rest) = token.strip_prefix('@') else {
            continue;
        };
        if rest.starts_with("http") || rest.contains('@') || rest.len() > 256 {
            continue;
        }
        let candidate = base_dir.join(rest);
        if !candidate.exists() {
            out.push(rest.to_string());
        }
    }
    out.sort();
    out.dedup();
    out
}
