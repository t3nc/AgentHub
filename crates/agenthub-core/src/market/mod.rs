//! 技能市场：只读公开接口 + 安装到中央仓库。
//!
//! 设计约束：
//! - **只有用户显式操作才出网**（搜索 / 看详情 / 安装），启动、扫描、刷新一律不联网；
//! - 网络层藏在 [`http::Http`] 后面，单测可离线注入假响应；
//! - 数据源做成 [`Registry`]，现有两个实现：默认的 SkillHub（`api.skillhub.cn`）
//!   与可选的 ClawHub（`clawhub.ai`）。两家协议不通用（端点、参数、响应信封都不同），
//!   所以换源换的是适配器，不是常量。
//!
//! 新增数据源的步骤：加一个 `impl Registry`，在 [`build_with`] 里分发，
//! 上层计划 / 安装 / 落盘逻辑不动。

pub mod archive;
pub mod clawhub;
pub mod http;
pub mod install;
pub mod json;
pub mod skillhub;

use crate::config::{AppConfig, MarketSource};
use crate::error::{Error, Result};
use crate::l10n::{Msg, t};
use crate::model::market::{MarketFile, MarketPage, MarketSkill, MarketSkillDetail, MarketSort};

/// 内置默认数据源。
pub const DEFAULT_SOURCE: MarketSource = MarketSource::SkillHub;

/// 默认源地址（SkillHub 的接口在 `api.` 子域，产品站 `skillhub.cn` 只是页面）。
pub const DEFAULT_REGISTRY: &str = "https://api.skillhub.cn";

/// 首屏与「加载更多」的默认批量。
pub const PAGE_SIZE_DEFAULT: usize = 50;
/// 单次请求上限：SkillHub 的 `pageSize` 只接受 ≤100，再大要自己拼多页。
pub const PAGE_SIZE_MAX: usize = 100;

/// JSON 响应上限（搜索结果一般几十 KB，2MB 已经非常宽松）。
pub const MAX_JSON_BYTES: usize = 2 * 1024 * 1024;
/// 技能包 ZIP 上限：解压前后都按这个量级兜底，防 zip bomb。
pub const MAX_ARCHIVE_BYTES: u64 = 20 * 1024 * 1024;
/// 单个技能包允许的最大文件数。
pub const MAX_ARCHIVE_FILES: usize = 300;

/// 一个市场数据源。
pub trait Registry {
    fn base_url(&self) -> &str;
    /// 搜索一页。`cursor` 为 `None` 表示第一页，否则是上一页返回的游标。
    fn search_page(&self, query: &str, cursor: Option<&str>, limit: usize) -> Result<MarketPage>;
    /// 按榜单浏览一页（关键词为空时用）。
    fn list_page(&self, sort: MarketSort, cursor: Option<&str>, limit: usize)
    -> Result<MarketPage>;
    /// 只取第一页的技能列表（不关心翻页的调用方 / 测试用）。
    fn search(&self, query: &str, limit: usize) -> Result<Vec<MarketSkill>> {
        Ok(self.search_page(query, None, limit)?.items)
    }
    fn list(&self, sort: MarketSort, limit: usize) -> Result<Vec<MarketSkill>> {
        Ok(self.list_page(sort, None, limit)?.items)
    }
    fn detail(&self, slug: &str) -> Result<MarketSkillDetail>;
    /// 技能包内文件清单（拿不到时返回空数组，安装会退回逐个读取）。
    fn files(&self, slug: &str, version: Option<&str>) -> Result<Vec<MarketFile>>;
    /// 整个包的 ZIP 字节；源不支持时返回 `Error::Unsupported`，由安装层降级。
    fn archive(&self, slug: &str, version: Option<&str>) -> Result<Vec<u8>>;
    /// 读取包内单个文本文件（ZIP 降级路径用）。
    fn file_text(&self, slug: &str, path: &str, version: Option<&str>) -> Result<String>;
}

/// 生效的市场目标：用哪家的协议、打哪个地址。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketTarget {
    pub source: MarketSource,
    pub base: String,
}

/// 解析生效目标。
///
/// 优先级：自定义地址的域名先反推数据源（老配置里可能只有 `registry_url`，
/// 例如 `https://clawhub.ai`，升级后必须仍按 ClawHub 协议说话）；
/// 推不出来才用显式配置的 `source`；都没有就跟随内置默认。
pub fn target(config: &AppConfig) -> Result<MarketTarget> {
    match config.market.base_url() {
        Some(raw) => {
            let base = normalize_base(&raw)?;
            let source = infer_source(&base).unwrap_or_else(|| config.market.source());
            Ok(MarketTarget { source, base })
        }
        None => {
            let source = config.market.source();
            Ok(MarketTarget {
                source,
                base: source.default_base().to_string(),
            })
        }
    }
}

/// 生效的源地址（等价于 `target(config)?.base`）。
pub fn resolve_base(config: &AppConfig) -> Result<String> {
    Ok(target(config)?.base)
}

/// 按域名反推数据源；认不出来返回 None。
pub fn infer_source(base: &str) -> Option<MarketSource> {
    let lower = base.to_ascii_lowercase();
    if lower.contains("clawhub") {
        Some(MarketSource::ClawHub)
    } else if lower.contains("skillhub") {
        Some(MarketSource::SkillHub)
    } else {
        None
    }
}

pub fn normalize_base(raw: &str) -> Result<String> {
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Ok(DEFAULT_SOURCE.default_base().to_string());
    }
    if !trimmed.starts_with("https://") {
        return Err(Error::Unsupported(t!(
            Msg::ErrMarketSourceHttps,
            value = trimmed
        )));
    }
    if trimmed.len() > 200 {
        return Err(Error::Unsupported(t!(Msg::ErrMarketSourceTooLong)));
    }
    Ok(trimmed.to_string())
}

/// 按配置构造数据源。
pub fn build(config: &AppConfig) -> Result<Box<dyn Registry>> {
    build_with(http::UreqHttp::new(), target(config)?)
}

/// 用给定网络层与目标构造（测试注入 `FakeHttp`）。
pub fn build_with<H: http::Http + 'static>(
    http: H,
    target: MarketTarget,
) -> Result<Box<dyn Registry>> {
    let MarketTarget { source, base } = target;
    let base = normalize_base(&base)?;
    Ok(match source {
        MarketSource::SkillHub => Box::new(skillhub::SkillHub::new(http, base)),
        MarketSource::ClawHub => Box::new(clawhub::ClawHub::new(http, base)),
    })
}

/// 极小的 percent-encoding：只放行 RFC 3986 unreserved 字符。
///
/// 不引 `url` crate —— 我们只需要编码查询串和路径段这两处。
pub fn encode(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for byte in raw.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(*byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_defaults_to_skillhub() {
        let config = AppConfig::default();
        assert_eq!(resolve_base(&config).unwrap(), DEFAULT_REGISTRY);
        assert_eq!(target(&config).unwrap().source, MarketSource::SkillHub);
    }

    #[test]
    fn explicit_source_picks_its_own_default_base() {
        let mut config = AppConfig::default();
        config.market.source = Some("clawhub".into());
        let resolved = target(&config).unwrap();
        assert_eq!(resolved.source, MarketSource::ClawHub);
        assert_eq!(resolved.base, "https://clawhub.ai");
    }

    #[test]
    fn legacy_registry_url_infers_source() {
        // 老配置只填了地址：必须按 ClawHub 协议说话，否则解析全错
        let mut config = AppConfig::default();
        config.market.registry_url = Some("https://clawhub.ai/".into());
        let resolved = target(&config).unwrap();
        assert_eq!(resolved.source, MarketSource::ClawHub);
        assert_eq!(resolved.base, "https://clawhub.ai");

        let mut config = AppConfig::default();
        config.market.registry_url = Some("https://api.skillhub.cn".into());
        assert_eq!(target(&config).unwrap().source, MarketSource::SkillHub);
    }

    #[test]
    fn unknown_base_falls_back_to_explicit_source() {
        let mut config = AppConfig::default();
        config.market.source = Some("clawhub".into());
        config.market.registry_url = Some("https://mirror.example.com".into());
        let resolved = target(&config).unwrap();
        assert_eq!(resolved.source, MarketSource::ClawHub);
        assert_eq!(resolved.base, "https://mirror.example.com");
    }

    #[test]
    fn invalid_source_string_falls_back_to_default() {
        let mut config = AppConfig::default();
        config.market.source = Some("nope".into());
        assert_eq!(target(&config).unwrap().source, DEFAULT_SOURCE);
    }

    #[test]
    fn base_rejects_plain_http() {
        let mut config = AppConfig::default();
        config.market.registry_url = Some("http://example.com".into());
        assert!(resolve_base(&config).is_err());
    }

    #[test]
    fn base_trims_trailing_slash() {
        assert_eq!(
            normalize_base("https://example.com/").unwrap(),
            "https://example.com"
        );
        // 空串等价于「跟随默认」
        assert_eq!(normalize_base("   ").unwrap(), DEFAULT_REGISTRY);
    }

    #[test]
    fn encode_keeps_unreserved_only() {
        assert_eq!(encode("a b/c?d"), "a%20b%2Fc%3Fd");
        assert_eq!(encode("a-b_c.d~e"), "a-b_c.d~e");
    }
}
