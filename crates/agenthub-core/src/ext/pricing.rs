//! 模型价格登记（Phase 4 的用量成本估算）。
//!
//! 只存「每 100 万 token 的单价」。价格既可由用户手填，也可从公开源导入——导入分
//! 「解析」与「写入」两段：这里只做纯函数解析，联网与落盘在 `api::pricing`。
//!
//! 未定价的模型一律输出 `None` 成本，界面上显示「未定价」而不是 `$0.00`——
//! 把缺失当成免费会让人误判花销。

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 价格单位：每 100 万 token。
pub const PRICE_UNIT: f64 = 1_000_000.0;

fn default_currency() -> String {
    "USD".into()
}

/// 一条模型价格。`provider` 为空表示「该模型的默认价」，匹配时优先精确 provider。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelPrice {
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// 非缓存输入单价（每百万 token）
    #[serde(default)]
    pub input: f64,
    #[serde(default)]
    pub output: f64,
    #[serde(default)]
    pub cache_read: f64,
    #[serde(default)]
    pub cache_write: f64,
    #[serde(default = "default_currency")]
    pub currency: String,
    /// `manual` / `models.dev` / `openrouter`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<i64>,
}

impl Default for ModelPrice {
    fn default() -> Self {
        Self {
            model: String::new(),
            provider: None,
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
            currency: default_currency(),
            source: None,
            updated_at: None,
        }
    }
}

impl ModelPrice {
    pub fn is_priced(&self) -> bool {
        self.input > 0.0 || self.output > 0.0 || self.cache_read > 0.0 || self.cache_write > 0.0
    }

    pub fn cost(&self, input: u64, cache_read: u64, cache_write: u64, output: u64) -> f64 {
        (self.input * input as f64
            + self.cache_read * cache_read as f64
            + self.cache_write * cache_write as f64
            + self.output * output as f64)
            / PRICE_UNIT
    }
}

/// 按 (模型, provider) 查价格：先精确 provider，再退回「无 provider 的默认价」。
pub fn lookup<'a>(
    prices: &'a [ModelPrice],
    model: &str,
    provider: Option<&str>,
) -> Option<&'a ModelPrice> {
    let want = model.trim().to_ascii_lowercase();
    let exact = prices.iter().find(|price| {
        price.model.trim().to_ascii_lowercase() == want
            && match (provider, price.provider.as_deref()) {
                (Some(left), Some(right)) => left.eq_ignore_ascii_case(right),
                (None, None) => true,
                _ => false,
            }
    });
    exact.or_else(|| {
        prices.iter().find(|price| {
            price.model.trim().to_ascii_lowercase() == want && price.provider.is_none()
        })
    })
}

/// 已知模型对应的 provider（价格表是最方便的来源，日志里只有 Codex 写了 provider）。
pub fn provider_of(prices: &[ModelPrice], model: &str) -> Option<String> {
    prices
        .iter()
        .find(|price| {
            price.model.trim().eq_ignore_ascii_case(model.trim()) && price.provider.is_some()
        })
        .and_then(|price| price.provider.clone())
}

/// 公开价格源。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PriceSource {
    /// https://models.dev/api.json —— 顶层 `{provider: {models: {model: {cost}}}}`
    ModelsDev,
    /// https://openrouter.ai/api/v1/models —— `{data: [{id, pricing: {prompt, ...}}]}`
    OpenRouter,
}

impl PriceSource {
    pub fn parse(raw: &str) -> Option<PriceSource> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "models_dev" | "modelsdev" | "models.dev" => Some(PriceSource::ModelsDev),
            "openrouter" => Some(PriceSource::OpenRouter),
            _ => None,
        }
    }

    pub fn key(&self) -> &'static str {
        match self {
            PriceSource::ModelsDev => "models_dev",
            PriceSource::OpenRouter => "openrouter",
        }
    }

    pub fn url(&self) -> &'static str {
        match self {
            PriceSource::ModelsDev => "https://models.dev/api.json",
            PriceSource::OpenRouter => "https://openrouter.ai/api/v1/models",
        }
    }
}

fn number(value: Option<&Value>) -> Option<f64> {
    let value = value?;
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|s| s.trim().parse::<f64>().ok()))
}

/// 解析公开源返回体。解析失败返回空列表（调用方给「没有可导入的模型」而不是崩溃）。
pub fn parse_source(source: PriceSource, body: &str, now: i64) -> Vec<ModelPrice> {
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return Vec::new();
    };
    match source {
        PriceSource::ModelsDev => parse_models_dev(&value, now),
        PriceSource::OpenRouter => parse_openrouter(&value, now),
    }
}

fn parse_models_dev(value: &Value, now: i64) -> Vec<ModelPrice> {
    let mut out = Vec::new();
    let Some(root) = value.as_object() else {
        return out;
    };
    for (provider, entry) in root {
        let Some(models) = entry.get("models").and_then(Value::as_object) else {
            continue;
        };
        for (model, def) in models {
            let Some(cost) = def.get("cost") else {
                continue;
            };
            let price = ModelPrice {
                model: model.clone(),
                provider: Some(provider.clone()),
                input: number(cost.get("input")).unwrap_or(0.0),
                output: number(cost.get("output")).unwrap_or(0.0),
                cache_read: number(cost.get("cache_read")).unwrap_or(0.0),
                cache_write: number(cost.get("cache_write")).unwrap_or(0.0),
                currency: "USD".into(),
                source: Some("models.dev".into()),
                updated_at: Some(now),
            };
            if price.is_priced() {
                out.push(price);
            }
        }
    }
    out.sort_by(|a, b| a.model.cmp(&b.model));
    out
}

fn parse_openrouter(value: &Value, now: i64) -> Vec<ModelPrice> {
    let mut out = Vec::new();
    let Some(items) = value.get("data").and_then(Value::as_array) else {
        return out;
    };
    for item in items {
        let Some(id) = item.get("id").and_then(Value::as_str) else {
            continue;
        };
        let Some(pricing) = item.get("pricing") else {
            continue;
        };
        // OpenRouter 的单价是「每 token 美元」，这里统一换算成每百万
        let scale = |key: &str| number(pricing.get(key)).unwrap_or(0.0) * PRICE_UNIT;
        let price = ModelPrice {
            model: id.to_string(),
            // OpenRouter 的 id 形如 `anthropic/claude-sonnet-4`，前缀就是 provider
            provider: id.split('/').next().map(|prefix| prefix.to_string()),
            input: scale("prompt"),
            output: scale("completion"),
            cache_read: scale("input_cache_read"),
            cache_write: scale("input_cache_write"),
            currency: "USD".into(),
            source: Some("openrouter".into()),
            updated_at: Some(now),
        };
        if price.is_priced() {
            out.push(price);
        }
    }
    out.sort_by(|a, b| a.model.cmp(&b.model));
    out
}
