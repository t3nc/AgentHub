//! 界面语言的领域侧实现。
//!
//! 与 `crate::ext::i18n`（技能 / MCP 介绍的内容翻译，Phase 5）不是一回事：
//! 这里只管「AgentHub 自己产出的文案」，GUI、CLI 共用同一份词典。
//!
//! 取词三条路：
//! - `str_of(msg)`：无参数，返回 `&'static str`，零分配（`label()` 这类签名不变）；
//! - `t!(msg, key = value)`：带占位符，返回 `String`；
//! - `text_for(locale, msg, args)`：显式指定语言，测试专用 —— 不读全局。

pub mod catalog;
pub mod fmt;
pub mod locale;

pub use catalog::Msg;
pub use locale::{Locale, current, detect_from_env, resolve, set_current};

/// 当前进程语言下的模板。
pub fn str_of(msg: Msg) -> &'static str {
    msg.template(current())
}

/// 当前进程语言下的成品文案（含占位符替换）。
pub fn text(msg: Msg, args: &[(&str, &str)]) -> String {
    text_for(current(), msg, args)
}

/// 指定语言下的成品文案。
///
/// 测试一律走这里：`cargo test` 并行跑，改全局语言会互相污染。
pub fn text_for(locale: Locale, msg: Msg, args: &[(&str, &str)]) -> String {
    fmt::interpolate(msg.template(locale), args).into_owned()
}

/// 指定语言下的模板（无参数场景，测试用）。
pub fn str_for(locale: Locale, msg: Msg) -> &'static str {
    msg.template(locale)
}

/// 取词宏：`t!(Msg::X)` / `t!(Msg::X, key = value)`。
///
/// `#[macro_export]` 是为了让 `agenthub-cli` 与 Tauri 壳层也能直接复用；
/// 宏体一律走 `$crate::l10n::…`，跨 crate 调用同样成立。
#[macro_export]
macro_rules! t {
    // 统一返回 `String`：调用点（`label` 字段、日志）几乎都是要 owned 的，
    // 少数只读场景直接用 `str_of()`，不要把两种返回类型混在一个宏里。
    ($msg:expr $(,)?) => {
        ::std::string::ToString::to_string($crate::l10n::str_of($msg))
    };
    ($msg:expr, $($key:ident = $value:expr),+ $(,)?) => {{
        let owned: ::std::vec::Vec<(&str, ::std::string::String)> =
            ::std::vec![$((stringify!($key), ::std::string::ToString::to_string(&$value))),+];
        let borrowed: ::std::vec::Vec<(&str, &str)> =
            owned.iter().map(|(key, value)| (*key, value.as_str())).collect();
        $crate::l10n::text($msg, &borrowed)
    }};
}

// CLI 与 Tauri 壳层也要用同一个宏，所以这里不限于 crate 内
pub use t;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 三种语言都有模板且占位符一致() {
        for msg in Msg::ALL {
            let hans = str_for(Locale::ZhHans, *msg);
            let hant = str_for(Locale::ZhHant, *msg);
            let en = str_for(Locale::En, *msg);
            assert!(!hans.is_empty(), "{} 缺少简体文案", msg.key());
            assert!(!hant.is_empty(), "{} 缺少繁体文案", msg.key());
            assert!(!en.is_empty(), "{} 缺少英文文案", msg.key());
        }
    }

    #[test]
    fn 占位符替换与未知占位符保留() {
        let text = text_for(
            Locale::ZhHans,
            Msg::DescWriteFile,
            &[("at", "/tmp/a"), ("bytes", "12")],
        );
        assert_eq!(text, "写入 /tmp/a (12 字节)");

        // 参数没给全时占位符原样留着，界面上一眼能看出来
        let text = text_for(Locale::En, Msg::DescWriteFile, &[("at", "/tmp/a")]);
        assert_eq!(text, "Write /tmp/a ({bytes} bytes)");
    }
}
