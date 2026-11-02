use std::sync::atomic::{AtomicU8, Ordering};

/// 界面语言。首发三种：英文 / 简体中文 / 繁体中文（台湾正体）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locale {
    ZhHans,
    ZhHant,
    En,
}

impl Locale {
    pub const ALL: [Locale; 3] = [Locale::ZhHans, Locale::ZhHant, Locale::En];

    /// 落 `config.json` 的取值；与前端 `LocaleMode` 的非 system 成员一一对应。
    pub fn code(self) -> &'static str {
        match self {
            Locale::ZhHans => "zh-Hans",
            Locale::ZhHant => "zh-Hant",
            Locale::En => "en",
        }
    }

    /// 前端 `<html lang>` / `Intl` 用的 BCP-47 标签。
    pub fn tag(self) -> &'static str {
        match self {
            Locale::ZhHans => "zh-CN",
            Locale::ZhHant => "zh-TW",
            Locale::En => "en-US",
        }
    }

    /// 严格解析：只认三种规范写法（大小写不敏感，`zh_CN` 这类下划线写法也认）。
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().replace('_', "-").as_str() {
            "zh-hans" | "zh-cn" | "zh-sg" | "zh" | "cn" => Some(Locale::ZhHans),
            "zh-hant" | "zh-tw" | "zh-hk" | "zh-mo" => Some(Locale::ZhHant),
            "en" | "en-us" | "en-gb" => Some(Locale::En),
            _ => None,
        }
    }

    /// 系统语言标签 → 界面语言。认不出来给英文（和前端 `resolveTag` 一致）。
    pub fn from_system_tag(tag: &str) -> Option<Self> {
        Self::parse(tag)
    }

    fn index(self) -> u8 {
        match self {
            Locale::ZhHans => 0,
            Locale::ZhHant => 1,
            Locale::En => 2,
        }
    }

    fn from_index(value: u8) -> Self {
        match value {
            1 => Locale::ZhHant,
            2 => Locale::En,
            _ => Locale::ZhHans,
        }
    }
}

/// 进程级当前语言。
///
/// 用 `AtomicU8` 而不是锁：读取发生在每次取文案时（包括扫描汇总、渲染表格），
/// 这里必须是零等待。写入只发生在启动与设置页切换，冲突概率可以忽略。
static CURRENT: AtomicU8 = AtomicU8::new(0);

pub fn current() -> Locale {
    Locale::from_index(CURRENT.load(Ordering::Relaxed))
}

pub fn set_current(locale: Locale) {
    CURRENT.store(locale.index(), Ordering::Relaxed);
}

/// 从环境变量推断：`AGENTHUB_LANG` 优先，其次 `LC_ALL` / `LC_MESSAGES` / `LANG`。
///
/// Windows 默认不设这些变量，CLI 会退到 `config.json` 里的 `locale`；
/// 不引入 `windows-sys` 的 Globalization 也能满足需求。
pub fn detect_from_env() -> Option<Locale> {
    for key in ["AGENTHUB_LANG", "LC_ALL", "LC_MESSAGES", "LANG"] {
        let Ok(raw) = std::env::var(key) else {
            continue;
        };
        // `zh_TW.UTF-8` → `zh-tw`
        let tag = raw.split('.').next().unwrap_or("").trim().to_string();
        if tag.is_empty() || tag == "C" || tag == "POSIX" {
            continue;
        }
        if let Some(locale) = Locale::from_system_tag(&tag) {
            return Some(locale);
        }
    }
    None
}

/// 启动时的语言解析：参数 > 环境变量 > 配置 > 英文。
///
/// `configured` 是 `config.json` 里的 `locale`（`None` / `"system"` 表示跟随系统）。
pub fn resolve(explicit: Option<Locale>, configured: Option<&str>) -> Locale {
    if let Some(locale) = explicit {
        return locale;
    }
    if let Some(locale) = detect_from_env() {
        return locale;
    }
    if let Some(raw) = configured {
        if raw.trim().eq_ignore_ascii_case("system") || raw.trim().is_empty() {
            return detect_from_env().unwrap_or(Locale::En);
        }
        if let Some(locale) = Locale::parse(raw) {
            return locale;
        }
    }
    Locale::En
}
