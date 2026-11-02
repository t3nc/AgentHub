//! 模型价格的读写入口：校验、落盘、公开源导入（预览 → 应用两段）。
//!
//! 与「技能 / MCP / 指令 / Provider 写盘」不同，这张表**不是**对 agent 目录的改写，
//! 因此不进 `ChangeSet`、不需要备份与回滚：它就是 `config.json` 里的一段登记，
//! 和 `providers` 同语义。这一点在 UI 与文档里都要写明，否则用户会问「为什么没有预览」。

use crate::l10n::{Msg, t};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::ext::pricing::{self, ModelPrice, PriceSource};
use crate::market::http::{Http, UreqHttp};
use crate::paths;

use super::AgentHub;

/// 公开源返回体能有多大：models.dev 全量约 1–2 MB，8 MB 足够且能挡住异常响应。
const MAX_BODY_BYTES: usize = 8 * 1024 * 1024;
/// 单价上限（每百万 token）。超过基本可以判定是填错了单位或币种。
const MAX_PRICE: f64 = 1_000_000.0;

#[derive(Debug, Clone, Serialize)]
pub struct PriceSourceView {
    pub key: String,
    pub label: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PricingView {
    pub prices: Vec<ModelPrice>,
    pub sources: Vec<PriceSourceView>,
    /// 口径提示：面板底部常驻，避免用户以为这是账单数据
    pub note: String,
}

/// 提交结构：允许 `provider` 为空（该模型的默认价）。
#[derive(Debug, Clone, Deserialize)]
pub struct ModelPriceInput {
    pub model: String,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub input: f64,
    #[serde(default)]
    pub output: f64,
    #[serde(default)]
    pub cache_read: f64,
    #[serde(default)]
    pub cache_write: f64,
    #[serde(default)]
    pub currency: Option<String>,
}

/// 价格口径说明（UI 与 CLI 共用）。取词放在函数里，切语言后重新拉一次就变。
pub fn pricing_note() -> String {
    t!(Msg::PricingNote)
}

pub fn sources() -> Vec<PriceSourceView> {
    [PriceSource::ModelsDev, PriceSource::OpenRouter]
        .into_iter()
        .map(|source| PriceSourceView {
            key: source.key().to_string(),
            label: match source {
                PriceSource::ModelsDev => "models.dev".to_string(),
                PriceSource::OpenRouter => "OpenRouter".to_string(),
            },
            url: source.url().to_string(),
        })
        .collect()
}

pub fn view(prices: &[ModelPrice]) -> PricingView {
    let mut sorted = prices.to_vec();
    sorted.sort_by(|a, b| a.model.cmp(&b.model));
    PricingView {
        prices: sorted,
        sources: sources(),
        note: pricing_note(),
    }
}

fn normalize_price(value: f64) -> Result<f64> {
    if !value.is_finite() || value < 0.0 {
        return Err(Error::Other(t!(Msg::ErrPriceInvalid)));
    }
    if value > MAX_PRICE {
        return Err(Error::Other(t!(
            Msg::ErrPriceTooHigh,
            value = value,
            max = MAX_PRICE
        )));
    }
    Ok(value)
}

fn clean_optional(raw: Option<&str>) -> Option<String> {
    raw.map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

/// 校验并归一用户提交的价格表（纯函数，便于单测）。
///
/// 同一 `(model, provider)` 只保留最后一条——表单里重复添加时以最后一次编辑为准。
pub fn build_prices(input: &[ModelPriceInput]) -> Result<Vec<ModelPrice>> {
    let mut out: Vec<ModelPrice> = Vec::with_capacity(input.len());
    for item in input {
        let model = item.model.trim();
        if model.is_empty() {
            return Err(Error::Other(t!(Msg::ErrModelNameEmpty)));
        }
        let provider = clean_optional(item.provider.as_deref());
        let price = ModelPrice {
            model: model.to_string(),
            provider: provider.clone(),
            input: normalize_price(item.input)?,
            output: normalize_price(item.output)?,
            cache_read: normalize_price(item.cache_read)?,
            cache_write: normalize_price(item.cache_write)?,
            currency: item
                .currency
                .as_deref()
                .map(|c| c.trim().to_ascii_uppercase())
                .filter(|c| !c.is_empty())
                .unwrap_or_else(|| "USD".into()),
            source: Some("manual".into()),
            updated_at: Some(paths::now_ts()),
        };
        match out
            .iter_mut()
            .find(|existing| existing.model == price.model && existing.provider == price.provider)
        {
            Some(existing) => *existing = price,
            None => out.push(price),
        }
    }
    Ok(out)
}

/// 拉取公开价格源。**只在用户显式触发时调用**，启动与扫描都不联网。
pub fn fetch(source: PriceSource) -> Result<Vec<ModelPrice>> {
    let url = source.url();
    let http = UreqHttp::new();
    let response = http.get(url, MAX_BODY_BYTES)?;
    if !(200..300).contains(&response.status) {
        return Err(Error::Other(t!(
            Msg::ErrPricingImportStatus,
            source = source.key(),
            status = response.status
        )));
    }
    let body = String::from_utf8_lossy(&response.body);
    let prices = pricing::parse_source(source, &body, paths::now_ts());
    if prices.is_empty() {
        return Err(Error::Other(t!(
            Msg::ErrPricingNoFields,
            source = source.key()
        )));
    }
    Ok(prices)
}

impl AgentHub {
    pub fn usage_pricing(&self) -> PricingView {
        view(&self.config.usage_pricing)
    }

    /// 整表保存（手填路径）。写入 `config.json`，成功后返回归一后的视图。
    pub fn save_usage_pricing(&mut self, input: &[ModelPriceInput]) -> Result<PricingView> {
        self.require_feature(crate::model::gateway::FeatureKey::Usage)?;
        let prices = build_prices(input)?;
        self.config.usage_pricing = prices;
        self.config.save()?;
        Ok(self.usage_pricing())
    }

    /// 只覆盖一条（CLI `pricing set` 用）。其它行的来源标记保持原样。
    pub fn upsert_usage_price(&mut self, input: &ModelPriceInput) -> Result<PricingView> {
        self.require_feature(crate::model::gateway::FeatureKey::Usage)?;
        let rows = build_prices(std::slice::from_ref(input))?;
        let Some(price) = rows.into_iter().next() else {
            return Ok(self.usage_pricing());
        };
        match self
            .config
            .usage_pricing
            .iter_mut()
            .find(|existing| existing.model == price.model && existing.provider == price.provider)
        {
            Some(existing) => *existing = price,
            None => self.config.usage_pricing.push(price),
        }
        self.config.save()?;
        Ok(self.usage_pricing())
    }

    pub fn clear_usage_pricing(&mut self) -> Result<PricingView> {
        self.require_feature(crate::model::gateway::FeatureKey::Usage)?;
        self.config.usage_pricing.clear();
        self.config.save()?;
        Ok(self.usage_pricing())
    }

    /// 导入预览：联网拉取并解析，**不写盘**。
    pub fn preview_usage_pricing(&self, source_key: &str) -> Result<Vec<ModelPrice>> {
        let source = PriceSource::parse(source_key)
            .ok_or_else(|| Error::Other(t!(Msg::ErrUnknownPriceSource, value = source_key)))?;
        fetch(source)
    }

    /// 应用导入：把候选行合并进现有表（同 `(model, provider)` 覆盖）。
    pub fn apply_usage_pricing(&mut self, rows: &[ModelPrice]) -> Result<PricingView> {
        self.require_feature(crate::model::gateway::FeatureKey::Usage)?;
        for row in rows {
            if row.model.trim().is_empty() {
                continue;
            }
            match self
                .config
                .usage_pricing
                .iter_mut()
                .find(|existing| existing.model == row.model && existing.provider == row.provider)
            {
                Some(existing) => *existing = row.clone(),
                None => self.config.usage_pricing.push(row.clone()),
            }
        }
        self.config.save()?;
        Ok(self.usage_pricing())
    }
}
