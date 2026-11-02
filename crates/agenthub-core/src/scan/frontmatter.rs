use std::collections::BTreeMap;
use std::path::Path;

use crate::paths;

#[derive(Debug, Clone, Default)]
pub struct Frontmatter {
    pub name: Option<String>,
    pub description: Option<String>,
    pub fields: BTreeMap<String, String>,
    pub body_preview: Option<String>,
}

const MAX_SKILL_MD: usize = 256 * 1024;

/// 读取技能目录里的 SKILL.md（兼容 skill.md / README.md）并解析 YAML frontmatter。
pub fn parse_skill_dir(dir: &Path) -> Frontmatter {
    for name in ["SKILL.md", "skill.md", "Skill.md", "README.md"] {
        let candidate = dir.join(name);
        if candidate.is_file()
            && let Ok(text) = paths::read_to_string_limited(&candidate, MAX_SKILL_MD)
        {
            let mut fm = parse_text(&text);
            if fm.name.is_none() {
                fm.name = dir.file_name().map(|n| n.to_string_lossy().to_string());
            }
            return fm;
        }
    }
    Frontmatter {
        name: dir.file_name().map(|n| n.to_string_lossy().to_string()),
        ..Default::default()
    }
}

pub fn parse_text(text: &str) -> Frontmatter {
    let mut fm = Frontmatter::default();
    // BOM 不是空白字符，会让首行的 `---` 判定失败；Windows 编辑器常写 BOM
    let normalized = crate::text::strip_bom(text).replace("\r\n", "\n");
    let mut lines = normalized.lines().peekable();

    if lines.peek().map(|l| l.trim()) != Some("---") {
        fm.body_preview = first_paragraph(&normalized);
        return fm;
    }
    lines.next();

    let mut current_key: Option<String> = None;
    let mut block: Vec<String> = Vec::new();
    let mut body_lines: Vec<&str> = Vec::new();
    let mut in_front = true;

    for line in lines.by_ref() {
        if in_front && line.trim() == "---" {
            in_front = false;
            continue;
        }
        if !in_front {
            body_lines.push(line);
            continue;
        }
        let indented = line.starts_with(' ') || line.starts_with('\t');
        if indented && current_key.is_some() {
            block.push(line.trim().to_string());
            continue;
        }
        if let Some(key) = current_key.take() {
            let value = block.join(" ").trim().to_string();
            if !value.is_empty() {
                fm.fields.insert(key, value);
            }
            block.clear();
        }
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        let key = k.trim().to_string();
        if key.is_empty() {
            continue;
        }
        let value = v.trim();
        if value.is_empty() || value == "|" || value == ">" || value == "|-" || value == ">-" {
            current_key = Some(key);
        } else {
            fm.fields.insert(key, trim_quotes(value).to_string());
        }
    }
    if let Some(key) = current_key.take() {
        let value = block.join(" ").trim().to_string();
        if !value.is_empty() {
            fm.fields.insert(key, value);
        }
    }

    fm.name = fm.fields.get("name").cloned();
    fm.description = fm
        .fields
        .get("description")
        .cloned()
        .or_else(|| first_paragraph(&body_lines.join("\n")));
    fm.body_preview = first_paragraph(&body_lines.join("\n"));
    fm
}

fn trim_quotes(value: &str) -> &str {
    let v = value.trim();
    if (v.starts_with('"') && v.ends_with('"') && v.len() >= 2)
        || (v.starts_with('\'') && v.ends_with('\'') && v.len() >= 2)
    {
        &v[1..v.len() - 1]
    } else {
        v
    }
}

fn first_paragraph(text: &str) -> Option<String> {
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("---") {
            continue;
        }
        let mut out: String = trimmed.chars().take(180).collect();
        if trimmed.chars().count() > 180 {
            out.push('…');
        }
        return Some(out);
    }
    None
}
