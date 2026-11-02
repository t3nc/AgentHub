//! 系统字体探测：给内嵌终端找 Nerd Font。
//!
//! 背景：oh-my-posh 一类提示符会输出大量 Nerd Font 私有区图标（U+E0B0、U+F0000 区），
//! xterm 的 font-family 里没有这些字形时只能画方块。这里把「用户已经装了什么」告诉前端。
//!
//! 三条替代方案都被放弃：
//! - 读注册表：value 名是 `family + 字重 + (TrueType)`，剥字符串不可靠；
//! - 调 `fc-list`：mac/Linux 未必装 fontconfig，还得处理输出编码；
//! - `document.fonts.check()`：本地字体名一律返回 true，分辨不出缺字。
//!
//! 实际做法：扫字体目录 → 文件名含 `nerd` 的才看 → `ttf-parser` 读 name 表拿 family。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use ttf_parser::Face;

/// 最多给前端几个候选。同一份字体会返回「缩写 + 全称」两个名字，
/// 所以这里要留够两个族的位置；再多也没意义，前面命中就够了。
const MAX_RESULTS: usize = 6;
/// 单个字体文件上限：字体本体 2–5 MB，超过这个数多半是 .ttc 合集，读进来也不划算。
const MAX_FONT_BYTES: u64 = 32 * 1024 * 1024;

/// 扫描只在第一次调用时发生（约几十毫秒），之后直接给缓存。
static CACHE: OnceLock<Vec<String>> = OnceLock::new();

/// 探测本机已安装的 Nerd Font family 名，按「终端里最合适」排序。
///
/// 任何异常都退化成空数组：字体探测失败不该让终端开不了。
#[tauri::command]
pub fn terminal_fonts() -> Vec<String> {
    CACHE.get_or_init(detect).clone()
}

fn detect() -> Vec<String> {
    let files = font_files();
    let families: Vec<String> = pick_representatives(files)
        .iter()
        .flat_map(|path| families_of(path))
        .collect();
    rank_families(families)
}

/// 各平台字体目录。
#[cfg(windows)]
fn font_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(windir) = std::env::var("WINDIR") {
        dirs.push(PathBuf::from(windir).join("Fonts"));
    }
    // 用户级安装（右键「为所有用户」以外的方式）落在 LocalAppData
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        dirs.push(
            PathBuf::from(local)
                .join("Microsoft")
                .join("Windows")
                .join("Fonts"),
        );
    }
    dirs
}

#[cfg(target_os = "macos")]
fn font_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![
        PathBuf::from("/Library/Fonts"),
        PathBuf::from("/System/Library/Fonts"),
    ];
    if let Ok(home) = std::env::var("HOME") {
        dirs.push(PathBuf::from(home).join("Library").join("Fonts"));
    }
    dirs
}

#[cfg(all(not(windows), not(target_os = "macos")))]
fn font_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![
        PathBuf::from("/usr/share/fonts"),
        PathBuf::from("/usr/local/share/fonts"),
    ];
    if let Ok(home) = std::env::var("HOME") {
        dirs.push(
            PathBuf::from(&home)
                .join(".local")
                .join("share")
                .join("fonts"),
        );
        dirs.push(PathBuf::from(&home).join(".fonts"));
    }
    dirs
}

/// 扫出所有「文件名里带 nerd」的字体文件。
///
/// 只按文件名粗筛、不解析全部字体：本机 Fonts 目录有几百个文件，
/// 解析每个的 name 表要几百毫秒，而 Nerd Font 的文件名一定带 `nerd`。
fn font_files() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for dir in font_dirs() {
        collect(&dir, 2, &mut out);
    }
    out.sort();
    out.dedup();
    out
}

fn collect(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            // Linux 的 /usr/share/fonts 下一层就是厂商目录，最多再下一层
            if depth > 0 {
                collect(&path, depth - 1, out);
            }
            continue;
        }
        if !is_font_file(&path) {
            continue;
        }
        let name = path
            .file_name()
            .map(|value| value.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        if name.contains("nerd") {
            out.push(path);
        }
    }
}

fn is_font_file(path: &Path) -> bool {
    let ext = path
        .extension()
        .map(|value| value.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    matches!(ext.as_str(), "ttf" | "otf" | "ttc")
}

/// 同一字族有一整套字重文件，解析前先按文件名分组，每组只留一个代表。
///
/// 本机装了 96 个 JetBrainsMono Nerd Font 文件，但只有 5 个不同的 family；
/// 不分组就得读 200 MB 字体数据。
fn pick_representatives(files: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut groups: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
    for path in files {
        groups.entry(style_key(&path)).or_default().push(path);
    }
    let mut out = Vec::new();
    for (_, mut members) in groups {
        members.sort_by_key(|path| style_rank(path));
        if let Some(path) = members.into_iter().next() {
            out.push(path);
        }
    }
    out
}

/// 已知字重/字形后缀。命中就从文件名里去掉，剩下的前缀当作「同一族」的键。
const STYLE_SUFFIXES: &[&str] = &[
    "extralightitalic",
    "extrabolditalic",
    "semibolditalic",
    "bolditalic",
    "mediumitalic",
    "lightitalic",
    "thinitalic",
    "blackitalic",
    "extralight",
    "extrabold",
    "semibold",
    "medium",
    "light",
    "regular",
    "italic",
    "oblique",
    "black",
    "bold",
    "thin",
];

fn style_key(path: &Path) -> String {
    let stem = path
        .file_stem()
        .map(|value| value.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let Some((base, suffix)) = stem.rsplit_once('-') else {
        return stem;
    };
    if STYLE_SUFFIXES.contains(&suffix) {
        base.to_string()
    } else {
        stem
    }
}

/// 代表文件优先级：Regular 最贴近终端默认字重，其次任意非 Bold。
fn style_rank(path: &Path) -> u8 {
    let stem = path
        .file_stem()
        .map(|value| value.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if stem.ends_with("-regular") {
        0
    } else if stem.contains("italic") || stem.contains("oblique") || stem.contains("bold") {
        2
    } else {
        1
    }
}

/// 读字体 name 表里的 family，返回去重后的若干等价写法。
///
/// 为什么要返回多个：Nerd Fonts v3 给同一份字体起了两套名字。
/// legacy family（id 1）是 `JetBrainsMono NFM` 这种缩写，typographic family（id 16）
/// 才是 `JetBrainsMono Nerd Font Mono`。用户在 Windows Terminal 里配的可能是任意一个，
/// 而 CSS 的 font-family 只能一个个试——都塞进列表，浏览器取第一个能匹配上的。
///
/// `.ttc` 只取第 0 个字体：主流发行包（Nerd Fonts 的 zip）都是单字体文件。
fn families_of(path: &Path) -> Vec<String> {
    let Ok(meta) = std::fs::metadata(path) else {
        return Vec::new();
    };
    if meta.len() > MAX_FONT_BYTES {
        return Vec::new();
    }
    let Ok(data) = std::fs::read(path) else {
        return Vec::new();
    };
    let Ok(face) = Face::parse(&data, 0) else {
        return Vec::new();
    };

    let mut out: Vec<String> = Vec::new();
    let names = face.names();
    for index in 0..names.len() {
        let Some(name) = names.get(index) else {
            continue;
        };
        if name.name_id != ttf_parser::name_id::FAMILY
            && name.name_id != ttf_parser::name_id::TYPOGRAPHIC_FAMILY
        {
            continue;
        }
        let Some(text) = name.to_string() else {
            continue;
        };
        let text = text.trim().to_string();
        if text.is_empty() || out.iter().any(|seen| seen.eq_ignore_ascii_case(&text)) {
            continue;
        }
        out.push(text);
    }
    out
}

/// family 名的优先级；`None` = 不适合终端。
///
/// 输入的 family 都来自「文件名含 nerd」的字体文件，所以这里不再判断是不是 Nerd Font，
/// 只看它在终端里合不合适。判断依据在**结尾**：
/// - `NFM` / `Nerd Font Mono`：图标单格宽，终端首选；
/// - `NFP` / `Propo`：图标按比例占两格，xterm 按单格排字，必错位，直接排除；
/// - `NF` / `Nerd Font`：双宽图标，能用但会挤列，排最后。
///
/// 不能用 `contains("mono")`：`JetBrainsMono NF` 的字体名自带 Mono 字样，
/// 但它是双宽的那一支。
fn nerd_rank(family: &str) -> Option<u8> {
    let lower = family.to_lowercase();
    if lower.contains("propo") || lower.ends_with("nfp") {
        return None;
    }
    if lower.ends_with("nfm") || lower.ends_with("nerd font mono") {
        Some(0)
    } else if lower.ends_with("nf") || lower.ends_with("nerd font") {
        Some(1)
    } else {
        Some(2)
    }
}

/// 排序 + 去重（忽略大小写）+ 截断。纯函数，单测覆盖。
fn rank_families(families: Vec<String>) -> Vec<String> {
    let mut scored: Vec<(u8, String)> = families
        .into_iter()
        .filter_map(|family| nerd_rank(&family).map(|rank| (rank, family)))
        .collect();
    // 同分时按名字排，保证结果稳定（换机器/换顺序不抖动）
    scored.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

    let mut seen = std::collections::BTreeSet::new();
    scored
        .into_iter()
        .filter(|(_, family)| seen.insert(family.to_lowercase()))
        .map(|(_, family)| family)
        .take(MAX_RESULTS)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ranked(names: &[&str]) -> Vec<String> {
        rank_families(names.iter().map(|name| name.to_string()).collect())
    }

    #[test]
    fn mono_beats_double_width() {
        // Nerd Fonts v3 的缩写命名
        assert_eq!(
            ranked(&["JetBrainsMono NF", "JetBrainsMono NFM"]),
            vec!["JetBrainsMono NFM", "JetBrainsMono NF"]
        );
        // v2 时代的全称命名，两套都要认
        assert_eq!(
            ranked(&["JetBrainsMono Nerd Font", "JetBrainsMono Nerd Font Mono"]),
            vec!["JetBrainsMono Nerd Font Mono", "JetBrainsMono Nerd Font"]
        );
    }

    #[test]
    fn propo_is_excluded() {
        assert_eq!(
            ranked(&[
                "JetBrainsMono NFP",
                "JetBrainsMono Nerd Font Propo",
                "CaskaydiaCove NF"
            ]),
            vec!["CaskaydiaCove NF"]
        );
    }

    #[test]
    fn duplicates_dropped_case_insensitive() {
        assert_eq!(
            ranked(&["FiraCode NFM", "firacode nfm"]),
            vec!["FiraCode NFM"]
        );
    }

    #[test]
    fn results_are_capped() {
        let names = [
            "A NFM", "B NFM", "C NFM", "D NFM", "E NFM", "F NFM", "G NFM", "H NFM",
        ];
        assert_eq!(ranked(&names).len(), MAX_RESULTS);
    }

    #[test]
    fn style_suffix_groups_weights() {
        let paths: Vec<PathBuf> = [
            "JetBrainsMonoNerdFontMono-Bold.ttf",
            "JetBrainsMonoNerdFontMono-Regular.ttf",
            "JetBrainsMonoNerdFontMono-LightItalic.ttf",
            "JetBrainsMonoNLNerdFontMono-Regular.ttf",
        ]
        .iter()
        .map(PathBuf::from)
        .collect();
        let picked = pick_representatives(paths);
        assert_eq!(picked.len(), 2, "两个不同字族应各留一个");
        assert!(
            picked.iter().any(|path| path
                .to_string_lossy()
                .ends_with("JetBrainsMonoNerdFontMono-Regular.ttf")),
            "同族里应选 Regular"
        );
    }

    /// 真机冒烟：本机装了 Nerd Font 才能看到东西，所以在 CI 上不跑。
    /// 手动执行：`cargo test -p agenthub-desktop fonts:: -- --ignored --nocapture`
    #[test]
    #[ignore = "依赖本机安装的字体，默认跳过"]
    fn detect_lists_local_nerd_fonts() {
        let files = font_files();
        println!("font_files: {}", files.len());
        let picked = pick_representatives(files);
        println!("picked: {}", picked.len());
        for path in &picked {
            println!(
                "  {:?} -> {:?}",
                path.file_name()
                    .map(|name| name.to_string_lossy().to_string()),
                families_of(path)
            );
        }
        let found = detect();
        println!("detected: {found:?}");
        assert!(
            found.iter().all(|name| nerd_rank(name).is_some()),
            "返回了不适合终端的字体：{found:?}"
        );
    }

    #[test]
    fn style_key_keeps_unknown_suffix() {
        assert_eq!(
            style_key(Path::new("SomeFont-Special.ttf")),
            "somefont-special"
        );
    }
}
