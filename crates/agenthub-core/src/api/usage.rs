//! Token 用量报告：把扫描得到的小时桶换算成六个时间范围，再按 agent / provider / 模型聚合。
//!
//! 范围换算（尤其「当日」与「24 小时」的区别）全部在 Rust 侧完成，CLI 与 GUI 共用
//! 同一实现，避免两边各写一套日期逻辑而给出不同的数。

use std::collections::BTreeMap;

use chrono::{Datelike, Local, TimeZone};
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::ext::pricing::{self, ModelPrice};
use crate::ext::usage::{self, ScanStats, SourceStats, UsageBucket};
use crate::l10n::{Msg, t};
use crate::paths;

use super::AgentHub;

/// 六档时间范围。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageRange {
    /// 本地自然日 00:00 起（含今天已发生的全部）
    Today,
    /// 滚动 24 小时
    Last24h,
    Days7,
    Days15,
    Days30,
    All,
}

impl UsageRange {
    pub fn parse(raw: &str) -> UsageRange {
        match raw.trim().to_ascii_lowercase().as_str() {
            "today" | "day" => UsageRange::Today,
            "24h" | "last24h" | "last_24h" => UsageRange::Last24h,
            "7d" | "days7" => UsageRange::Days7,
            "15d" | "days15" => UsageRange::Days15,
            "30d" | "days30" => UsageRange::Days30,
            "all" => UsageRange::All,
            _ => UsageRange::Days7,
        }
    }

    pub fn key(&self) -> &'static str {
        match self {
            UsageRange::Today => "today",
            UsageRange::Last24h => "last24h",
            UsageRange::Days7 => "days7",
            UsageRange::Days15 => "days15",
            UsageRange::Days30 => "days30",
            UsageRange::All => "all",
        }
    }

    pub fn label(&self) -> String {
        match self {
            UsageRange::Today => t!(Msg::UsageRangeToday),
            UsageRange::Last24h => t!(Msg::UsageRangeLast24h),
            UsageRange::Days7 => t!(Msg::UsageRangeDays7),
            UsageRange::Days15 => t!(Msg::UsageRangeDays15),
            UsageRange::Days30 => t!(Msg::UsageRangeDays30),
            UsageRange::All => t!(Msg::UsageRangeAll),
        }
    }

    /// 起始时间（epoch 秒，本地时区换算）；`全部` 返回 None。
    pub fn start_secs(&self, now: i64) -> Option<i64> {
        match self {
            UsageRange::Today => Some(local_day_start(now)),
            UsageRange::Last24h => Some(now - 86_400),
            UsageRange::Days7 => Some(now - 7 * 86_400),
            UsageRange::Days15 => Some(now - 15 * 86_400),
            UsageRange::Days30 => Some(now - 30 * 86_400),
            UsageRange::All => None,
        }
    }

    /// 桶的展示粒度：当日 / 24 小时按小时，其余按天。
    pub fn granularity(&self) -> Granularity {
        match self {
            UsageRange::Today | UsageRange::Last24h => Granularity::Hour,
            _ => Granularity::Day,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Granularity {
    Hour,
    Day,
}

pub fn local_day_start(epoch_secs: i64) -> i64 {
    let Some(dt) = Local.timestamp_opt(epoch_secs, 0).single() else {
        return epoch_secs;
    };
    Local
        .with_ymd_and_hms(dt.year(), dt.month(), dt.day(), 0, 0, 0)
        .single()
        .map(|start| start.timestamp())
        .unwrap_or(epoch_secs)
}

/// 聚合结果：既有四项计量，也有调用数与（可能的）成本。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UsageTotals {
    pub input: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub output: u64,
    pub calls: u64,
    pub total: u64,
    /// 已定价部分的成本；模型全部未定价时为 None
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,
    /// 未定价模型的调用数：> 0 时界面上成本要标「部分未定价」
    pub unpriced_calls: u64,
    pub currency: String,
}

impl UsageTotals {
    /// 累加一个桶。公开给 CLI 做「按时间线求和」这类展示期派生。
    pub fn add_bucket(&mut self, bucket: &UsageBucket) {
        self.input = self.input.saturating_add(bucket.input);
        self.cache_read = self.cache_read.saturating_add(bucket.cache_read);
        self.cache_write = self.cache_write.saturating_add(bucket.cache_write);
        self.output = self.output.saturating_add(bucket.output);
        self.calls = self.calls.saturating_add(bucket.calls as u64);
        self.total = self
            .input
            .saturating_add(self.cache_read)
            .saturating_add(self.cache_write)
            .saturating_add(self.output);
    }

    fn merge(&mut self, other: &UsageTotals) {
        self.input = self.input.saturating_add(other.input);
        self.cache_read = self.cache_read.saturating_add(other.cache_read);
        self.cache_write = self.cache_write.saturating_add(other.cache_write);
        self.output = self.output.saturating_add(other.output);
        self.calls = self.calls.saturating_add(other.calls);
        self.total = self.total.saturating_add(other.total);
        self.unpriced_calls = self.unpriced_calls.saturating_add(other.unpriced_calls);
        if let Some(cost) = other.cost {
            self.cost = Some(self.cost.unwrap_or(0.0) + cost);
        }
    }
}

/// 一个时间桶 × 模型 × provider。前端用它做全部筛选与排序。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageCell {
    /// 桶起点（epoch 秒）：小时档是整点，天档是本地 00:00
    pub bucket: i64,
    pub agent: String,
    pub model: String,
    pub provider: Option<String>,
    pub input: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub output: u64,
    pub calls: u64,
}

/// 按某个维度聚合出来的一行。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageGroup {
    pub key: String,
    pub label: String,
    #[serde(flatten)]
    pub totals: UsageTotals,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageReport {
    pub generated_at: i64,
    pub range: UsageRange,
    pub range_label: String,
    pub granularity: Granularity,
    /// 统计区间起点（epoch 秒）；全部档为 None
    pub start: Option<i64>,
    pub totals: UsageTotals,
    pub cells: Vec<UsageCell>,
    pub by_agent: Vec<UsageGroup>,
    pub by_provider: Vec<UsageGroup>,
    pub by_model: Vec<UsageGroup>,
    pub sources: Vec<SourceStats>,
    pub stats: ScanStats,
    pub prices: Vec<ModelPrice>,
    /// 口径说明，UI 与 CLI 直接展示，避免三处文案漂移
    pub note: String,
}

/// 「provider 未知」的**稳定比对键**。
///
/// 只用于分组与筛选比对；展示文案一律走 `Msg::UsageUnregisteredProvider`
/// （前端对应 `lib/usage.ts` 的 `unregisteredProvider()`）。两者不能混用：
/// 拿本地化文案当 key，切换语言后分组会**静默**对不上。
pub const UNREGISTERED_PROVIDER_ID: &str = "__unregistered__";

/// 「provider 未知」的展示文案。
pub fn unregistered_provider_label() -> String {
    t!(Msg::UsageUnregisteredProvider)
}

/// 模型名的展示值：解析器给的稳定哨兵换成当前语言，其余原样。
fn model_label(raw: &str) -> String {
    if raw == usage::UNKNOWN_MODEL_ID {
        t!(Msg::UsageUnknownModel)
    } else {
        raw.to_string()
    }
}

fn currency_of(prices: &[ModelPrice]) -> String {
    prices
        .first()
        .map(|price| price.currency.clone())
        .unwrap_or_else(|| "USD".into())
}

/// 把扫描结果换算成报告。公开给测试注入沙箱扫描结果（生产入口是 `usage_report`）。
pub fn report_from(
    range: UsageRange,
    scan: usage::UsageScan,
    prices: &[ModelPrice],
) -> UsageReport {
    let start = range.start_secs(paths::now_ts());
    assemble(range, start, scan, prices)
}

/// 实际聚合。`start` 由调用方给：`narrow` 传 None（cells 已在范围内，再按起点过滤
/// 会把「天桶折算成 00:00」的那天误删）。
fn assemble(
    range: UsageRange,
    start: Option<i64>,
    scan: usage::UsageScan,
    prices: &[ModelPrice],
) -> UsageReport {
    let now = paths::now_ts();
    let granularity = range.granularity();
    let currency = currency_of(prices);

    let mut cells: BTreeMap<(i64, String, String, String), UsageCell> = BTreeMap::new();
    for (agent, bucket) in &scan.buckets {
        if let Some(start) = start
            && bucket.hour_utc < start
        {
            continue;
        }
        let bucket_start = match granularity {
            Granularity::Hour => bucket.hour_utc,
            Granularity::Day => local_day_start(bucket.hour_utc),
        };
        // provider 优先用日志里的（Codex 有 model_provider），否则按价格表推断；
        // 两者都没有时归到「未登记」——这是推断而非日志事实，界面上会写清。
        let provider = bucket
            .provider
            .clone()
            .or_else(|| pricing::provider_of(prices, &bucket.model));
        let key = (
            bucket_start,
            agent.clone(),
            bucket.model.clone(),
            provider.clone().unwrap_or_default(),
        );
        let cell = cells.entry(key).or_insert_with(|| UsageCell {
            bucket: bucket_start,
            agent: agent.clone(),
            // 哨兵在出站时换成当前语言（缓存里永远存哨兵）
            model: model_label(&bucket.model),
            provider: provider.clone(),
            input: 0,
            cache_read: 0,
            cache_write: 0,
            output: 0,
            calls: 0,
        });
        cell.input = cell.input.saturating_add(bucket.input);
        cell.cache_read = cell.cache_read.saturating_add(bucket.cache_read);
        cell.cache_write = cell.cache_write.saturating_add(bucket.cache_write);
        cell.output = cell.output.saturating_add(bucket.output);
        cell.calls = cell.calls.saturating_add(bucket.calls as u64);
    }

    let mut totals = UsageTotals {
        currency: currency.clone(),
        ..UsageTotals::default()
    };
    let mut by_agent: BTreeMap<String, UsageTotals> = BTreeMap::new();
    let mut by_provider: BTreeMap<String, UsageTotals> = BTreeMap::new();
    let mut by_model: BTreeMap<String, UsageTotals> = BTreeMap::new();

    for cell in cells.values() {
        let bucket = UsageBucket {
            hour_utc: cell.bucket,
            model: cell.model.clone(),
            provider: cell.provider.clone(),
            input: cell.input,
            cache_read: cell.cache_read,
            cache_write: cell.cache_write,
            output: cell.output,
            calls: cell.calls.min(u32::MAX as u64) as u32,
        };
        let price = pricing::lookup(prices, &cell.model, cell.provider.as_deref());
        let mut group = UsageTotals {
            currency: currency.clone(),
            ..UsageTotals::default()
        };
        group.add_bucket(&bucket);
        match price {
            Some(price) => {
                let cost = price.cost(cell.input, cell.cache_read, cell.cache_write, cell.output);
                group.cost = Some(cost);
            }
            None => group.unpriced_calls = group.calls,
        }

        totals.merge(&group);
        by_agent
            .entry(cell.agent.clone())
            .or_default()
            .merge(&group);
        by_provider
            .entry(
                cell.provider
                    .clone()
                    .unwrap_or_else(|| UNREGISTERED_PROVIDER_ID.to_string()),
            )
            .or_default()
            .merge(&group);
        by_model
            .entry(cell.model.clone())
            .or_default()
            .merge(&group);
    }

    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    for stats in &scan.sources {
        labels.insert(stats.source.key.clone(), stats.source.label.clone());
    }

    let to_groups = |map: BTreeMap<String, UsageTotals>, use_label: bool| {
        let mut out: Vec<UsageGroup> = map
            .into_iter()
            .map(|(key, mut totals)| {
                totals.currency = currency.clone();
                let label = if key == UNREGISTERED_PROVIDER_ID {
                    // 比对键是哨兵，展示走词典
                    unregistered_provider_label()
                } else if use_label {
                    labels.get(&key).cloned().unwrap_or_else(|| key.clone())
                } else {
                    key.clone()
                };
                UsageGroup { key, label, totals }
            })
            .collect();
        out.sort_by_key(|group| std::cmp::Reverse(group.totals.total));
        out
    };

    UsageReport {
        generated_at: now,
        range,
        range_label: range.label(),
        granularity,
        start,
        totals,
        cells: cells.into_values().collect(),
        by_agent: to_groups(by_agent, true),
        by_provider: to_groups(by_provider, false),
        by_model: to_groups(by_model, false),
        sources: scan.sources,
        stats: scan.stats,
        prices: prices.to_vec(),
        note: usage::totals_note(),
    }
}

impl AgentHub {
    /// 生成用量报告。`refresh = true` 时丢弃按文件缓存强制重算。
    ///
    /// 「Token 统计」被禁用时直接报错：解析几百 MB 会话日志是真实开销，
    /// 不应该因为某个后台调用而继续跑。
    pub fn usage_report(&self, range: UsageRange, refresh: bool) -> Result<UsageReport> {
        self.require_feature(crate::model::gateway::FeatureKey::Usage)?;
        if refresh {
            self.db.clear_usage_cache()?;
        }
        let scan = usage::scan(&self.db)?;
        Ok(report_from(range, scan, &self.config.usage_pricing))
    }
}

/// 按 agent / 模型 / provider 收窄报告。
///
/// 走 cells 重新聚合，所以 `totals` 与三个分组都是过滤后的真实值，不是「只藏掉几行」。
/// CLI 用它实现 `--agent / --model / --provider`；前端在 TypeScript 侧做同一件事。
pub fn narrow(
    report: &UsageReport,
    agent: Option<&str>,
    model: Option<&str>,
    provider: Option<&str>,
) -> UsageReport {
    let agent = agent.map(|value| value.trim().to_ascii_lowercase());
    let model = model.map(|value| value.trim().to_ascii_lowercase());
    let provider = provider.map(|value| value.trim().to_ascii_lowercase());
    let buckets = report
        .cells
        .iter()
        .filter(|cell| {
            agent
                .as_deref()
                .is_none_or(|want| cell.agent.to_ascii_lowercase() == want)
        })
        .filter(|cell| {
            model
                .as_deref()
                .is_none_or(|want| cell.model.to_ascii_lowercase() == want)
        })
        .filter(|cell| {
            provider
                .as_deref()
                .is_none_or(|want| match cell.provider.as_deref() {
                    Some(value) => value.to_ascii_lowercase() == want,
                    // 「未登记」这一组：稳定哨兵与当前语言的展示名都认
                    None => {
                        want == UNREGISTERED_PROVIDER_ID
                            || unregistered_provider_label().to_ascii_lowercase() == want
                    }
                })
        })
        .map(|cell| {
            (
                cell.agent.clone(),
                UsageBucket {
                    hour_utc: cell.bucket,
                    model: cell.model.clone(),
                    provider: cell.provider.clone(),
                    input: cell.input,
                    cache_read: cell.cache_read,
                    cache_write: cell.cache_write,
                    output: cell.output,
                    calls: cell.calls.min(u32::MAX as u64) as u32,
                },
            )
        })
        .collect();
    let scan = usage::UsageScan {
        sources: report.sources.clone(),
        buckets,
        stats: report.stats.clone(),
    };
    let mut out = assemble(report.range, None, scan, &report.prices);
    // 区间起点是展示信息，不能因为重算而丢掉（否则界面会显示「最早一条起」）
    out.start = report.start;
    out
}
