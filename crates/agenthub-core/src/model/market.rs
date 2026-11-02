use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::agent::AgentId;
use crate::l10n::{self, Msg, t};

/// 市场里的一条技能（列表 / 搜索结果的统一形态）。
///
/// 各端点返回的 JSON 结构并不一致（列表用 `items[]` + 嵌套 `stats`，搜索用
/// `results[]` 且把 `downloads` 铺在顶层），解析层负责抹平差异，这里只保留
/// UI 真正要用的字段。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketSkill {
    /// 单段标识，直接用 `skills/{slug}` 取详情（不能用 `owner/slug`）
    pub slug: String,
    pub display_name: String,
    pub summary: String,
    pub owner: String,
    pub version: Option<String>,
    pub downloads: u64,
    pub installs: u64,
    pub stars: u64,
    /// 秒级时间戳；缺失时 None（UI 显示「—」）
    pub updated_at: Option<i64>,
    pub topics: Vec<String>,
    /// 审核结论：可疑（允许安装但显著警告）
    pub suspicious: bool,
    /// 审核结论：已确认恶意（直接拒绝安装）
    pub malware_blocked: bool,
    pub verdict: Option<String>,
    /// 官方发布（列表页给个徽标）
    pub official: bool,
}

impl MarketSkill {
    /// 装到本地时用的目录名：只保留文件系统友好的字符，且不允许为空。
    pub fn local_name(&self) -> String {
        let raw = if self.slug.trim().is_empty() {
            &self.display_name
        } else {
            &self.slug
        };
        let cleaned: String = raw
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                    c
                } else {
                    '-'
                }
            })
            .collect();
        let trimmed = cleaned.trim_matches(['-', '.']).to_string();
        if trimmed.is_empty() {
            "market-skill".into()
        } else {
            trimmed
        }
    }
}

/// 一页市场结果。
///
/// 两家源的分页语义完全不同（SkillHub 用页码，ClawHub 用 `nextCursor`，
/// ClawHub 的搜索还没有游标），所以 `cursor` 对上层**不透明**：
/// 前端只负责把上一次拿到的值原样回传，由适配器自己解释。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketPage {
    pub items: Vec<MarketSkill>,
    /// 取下一批用的游标；`None` = 没有下一页
    pub cursor: Option<String>,
    pub has_more: bool,
}

/// 技能包里的单个文件（来自 `versions/{version}` 的 `files[]`）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketFile {
    pub path: String,
    pub size: u64,
    #[serde(default)]
    pub content_type: Option<String>,
}

/// 详情页：技能元信息 + 正文 + 文件清单 + 审核说明。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketSkillDetail {
    pub skill: MarketSkill,
    /// SKILL.md 正文（前端按纯文本渲染，不解析 HTML）
    pub description: String,
    pub license: Option<String>,
    pub changelog: Option<String>,
    pub moderation_summary: Option<String>,
    pub reason_codes: Vec<String>,
    pub homepage: Option<String>,
    pub files: Vec<MarketFile>,
}

impl MarketSkillDetail {
    /// 安装前的硬性拦截条件：只有「确认恶意 / 被下架」才拒绝。
    pub fn blocked_reason(&self) -> Option<String> {
        if self.skill.malware_blocked {
            return Some(t!(Msg::ErrMarketMalicious, name = self.skill.display_name));
        }
        None
    }
}

/// 本机可安装目标：只有「检测到可用」的 agent 才会出现在勾选列表里。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketAgent {
    pub id: AgentId,
    pub display_name: String,
    /// 主技能目录（链接会建在这里）
    pub skills_dir: Option<PathBuf>,
    /// CLI 在 PATH 里，或配置目录存在
    pub detected: bool,
    /// `detected && skills_dir.is_some()`，UI 只让这一批可勾选
    pub available: bool,
    /// 不可用原因，直接展示给用户
    pub reason: Option<String>,
}

/// 一次安装请求。`agents` 为空时由调用方报错（错在 UI 比错在落盘更安全）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketInstallInput {
    pub slug: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub agents: Vec<AgentId>,
}

/// 排序方式。ClawHub 的 `sort` 取值见 `slug()`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarketSort {
    Recommended,
    Downloads,
    Updated,
}

impl MarketSort {
    pub fn slug(&self) -> &'static str {
        match self {
            MarketSort::Recommended => "recommended",
            MarketSort::Downloads => "downloads",
            MarketSort::Updated => "updated",
        }
    }

    pub fn label(&self) -> &'static str {
        l10n::str_of(match self {
            MarketSort::Recommended => Msg::SortRecommended,
            MarketSort::Downloads => Msg::SortDownloads,
            MarketSort::Updated => Msg::SortUpdated,
        })
    }
}
