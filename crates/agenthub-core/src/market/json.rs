//! 各家市场 JSON 的共用取值工具。
//!
//! 同一个概念在不同端点、不同源里的位置并不一致（列表把 `downloads` 铺在顶层、
//! 详情包在 `stats` 里，ClawHub 与 SkillHub 的命名也不完全一样），
//! 所以解析一律走「按候选路径逐层取值，缺字段退默认」，
//! 不让一次 API 微调把整页打挂。
//!
//! 这里只放无状态的纯函数；任何一个源都不该往这里塞自己的业务语义。

use serde_json::Value;

/// 按候选路径逐层取值，返回原生 `Value`。
pub(crate) fn dig<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut cursor = value;
    for key in path {
        cursor = cursor.get(*key)?;
    }
    if cursor.is_null() { None } else { Some(cursor) }
}

pub(crate) fn pick_str(value: &Value, paths: &[&[&str]]) -> Option<String> {
    paths.iter().find_map(|path| {
        dig(value, path)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    })
}

/// 先在 `primary` 里找，找不到再退到 `fallback`。
pub(crate) fn pick_str2(primary: &Value, fallback: &Value, paths: &[&[&str]]) -> Option<String> {
    pick_str(primary, paths).or_else(|| pick_str(fallback, paths))
}

pub(crate) fn pick_u64(value: &Value, paths: &[&[&str]]) -> Option<u64> {
    paths.iter().find_map(|path| {
        dig(value, path).and_then(|v| {
            v.as_u64()
                .or_else(|| v.as_f64().map(|f| f.max(0.0) as u64))
                .or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
        })
    })
}

pub(crate) fn pick_bool2(primary: &Value, fallback: &Value, paths: &[&[&str]]) -> Option<bool> {
    paths.iter().find_map(|path| {
        dig(primary, path)
            .and_then(Value::as_bool)
            .or_else(|| dig(fallback, path).and_then(Value::as_bool))
    })
}

pub(crate) fn pick_str_list(value: &Value, path: &[&str]) -> Vec<String> {
    dig(value, path)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn pick_str_list_multi(values: &[&Value], paths: &[&[&str]]) -> Vec<String> {
    for path in paths {
        for value in values {
            let found = pick_str_list(value, path);
            if !found.is_empty() {
                return found;
            }
        }
    }
    Vec::new()
}

/// 接口用的是毫秒时间戳；小于 1e11 的当作已经是秒。
pub(crate) fn ms_to_secs(ms: u64) -> i64 {
    if ms > 100_000_000_000 {
        (ms / 1000) as i64
    } else {
        ms as i64
    }
}

/// 把多行摘要压成一行并截断（UI 只展示一行）。
pub(crate) fn one_line(raw: &str, max_chars: usize) -> String {
    let flat: String = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= max_chars {
        return flat;
    }
    let clipped: String = flat.chars().take(max_chars).collect();
    format!("{clipped}…")
}

/// 相对链接补成绝对地址。
pub(crate) fn absolutize(base: &str, raw: &str) -> String {
    if raw.starts_with("http://") || raw.starts_with("https://") {
        raw.to_string()
    } else if raw.starts_with('/') {
        format!("{base}{raw}")
    } else {
        format!("{base}/{raw}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn dig_and_pick_skip_null_and_blank() {
        let value = json!({ "a": { "b": null }, "c": "  ", "d": " ok " });
        assert!(dig(&value, &["a", "b"]).is_none());
        assert_eq!(pick_str(&value, &[&["c"], &["d"]]).as_deref(), Some("ok"));
    }

    #[test]
    fn ms_to_secs_handles_both_units() {
        assert_eq!(ms_to_secs(1_793_379_562_875), 1_793_379_562);
        assert_eq!(ms_to_secs(1_793_379_562), 1_793_379_562);
    }

    #[test]
    fn one_line_flattens_and_clips() {
        assert_eq!(one_line("a\n b\tc", 32), "a b c");
        assert_eq!(one_line("abcd", 3), "abc…");
    }

    #[test]
    fn absolutize_keeps_absolute_and_joins_relative() {
        assert_eq!(absolutize("https://x", "https://y/z"), "https://y/z");
        assert_eq!(absolutize("https://x", "/a"), "https://x/a");
        assert_eq!(absolutize("https://x", "a"), "https://x/a");
    }
}
