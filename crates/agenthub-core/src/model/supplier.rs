//! 内置供应商目录（对标 CC Switch 的「添加供应商」列表）。
//!
//! 两条硬规矩：
//! 1. **地址宁缺勿错**。写错的 base_url 比没有更糟 —— 用户会以为是自己 key 的问题。
//!    `verified = false` 表示这条地址没能用官方文档确认，界面必须显式提示。
//! 2. **只预置「确定能用 API Key 直连」的端点**。走 OAuth / 账号登录的
//!    （Claude Desktop Official、GitHub Copilot、ChatGPT 订阅）不在本目录，
//!    用户要接就自己填自定义供应商。
//!
//! 默认模型只放 1–3 个代表值：真实清单由「获取模型列表」从上游拉，
//! 预置太多反而会随厂商改名而过期。

use crate::model::gateway::GatewayProtocol;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SupplierCategory {
    /// 模型官方：协议与地址都由官方直接提供
    Official,
    /// 国内模型厂商的 OpenAI 兼容端点
    ModelVendor,
    /// 聚合平台：一个 key 转发多家
    Aggregator,
}

impl SupplierCategory {
    pub fn label(self) -> &'static str {
        crate::l10n::str_of(match self {
            SupplierCategory::Official => crate::l10n::Msg::SupplierOfficial,
            SupplierCategory::ModelVendor => crate::l10n::Msg::SupplierVendor,
            SupplierCategory::Aggregator => crate::l10n::Msg::SupplierAggregator,
        })
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SupplierDef {
    pub id: &'static str,
    pub name: &'static str,
    /// 厂商名，用于卡片上的第二行与搜索
    pub vendor: &'static str,
    pub category: SupplierCategory,
    pub website: &'static str,
    pub base_url: &'static str,
    pub protocol: GatewayProtocol,
    pub default_model: &'static str,
    pub models: &'static [&'static str],
    /// 官方文档地址：地址对不对以它为准
    pub source_url: &'static str,
    /// false = 没能用官方文档确认，界面要提示「地址待核实」
    pub verified: bool,
}

/// 全部内置渠道。顺序即界面顺序（官方 → 国内厂商 → 聚合）。
pub const SUPPLIERS: &[SupplierDef] = &[
    SupplierDef {
        id: "anthropic",
        name: "Anthropic",
        vendor: "Anthropic",
        category: SupplierCategory::Official,
        website: "https://www.anthropic.com",
        base_url: "https://api.anthropic.com",
        protocol: GatewayProtocol::Anthropic,
        default_model: "claude-sonnet-4-5",
        models: &["claude-sonnet-4-5", "claude-opus-4-1"],
        source_url: "https://docs.anthropic.com/en/api/models-list",
        verified: true,
    },
    SupplierDef {
        id: "openai",
        name: "OpenAI",
        vendor: "OpenAI",
        category: SupplierCategory::Official,
        website: "https://platform.openai.com",
        base_url: "https://api.openai.com/v1",
        protocol: GatewayProtocol::ChatCompletions,
        default_model: "gpt-6.1",
        models: &["gpt-6.1"],
        source_url: "https://platform.openai.com/docs/api-reference/models",
        verified: true,
    },
    SupplierDef {
        id: "google",
        name: "Google Gemini",
        vendor: "Google",
        category: SupplierCategory::Official,
        website: "https://ai.google.dev",
        // Gemini 的 OpenAI 兼容层，不是原生 generateContent 接口
        base_url: "https://generativelanguage.googleapis.com/v1beta/openai",
        protocol: GatewayProtocol::ChatCompletions,
        default_model: "gemini-3.0-pro",
        models: &["gemini-3.0-pro", "gemini-3.0-flash"],
        source_url: "https://ai.google.dev/gemini-api/docs/openai",
        verified: true,
    },
    SupplierDef {
        id: "xai",
        name: "xAI Grok",
        vendor: "xAI",
        category: SupplierCategory::Official,
        website: "https://x.ai",
        base_url: "https://api.x.ai/v1",
        protocol: GatewayProtocol::ChatCompletions,
        default_model: "grok-4",
        models: &["grok-4"],
        source_url: "https://docs.x.ai/docs/api-reference",
        verified: true,
    },
    SupplierDef {
        id: "deepseek",
        name: "DeepSeek",
        vendor: "DeepSeek",
        category: SupplierCategory::ModelVendor,
        website: "https://platform.deepseek.com",
        base_url: "https://api.deepseek.com/v1",
        protocol: GatewayProtocol::ChatCompletions,
        default_model: "deepseek-chat",
        models: &["deepseek-chat", "deepseek-reasoner"],
        source_url: "https://api-docs.deepseek.com/",
        verified: true,
    },
    SupplierDef {
        id: "kimi",
        name: "Kimi（Moonshot）",
        vendor: "Moonshot",
        category: SupplierCategory::ModelVendor,
        website: "https://platform.moonshot.cn",
        base_url: "https://api.moonshot.cn/v1",
        protocol: GatewayProtocol::ChatCompletions,
        default_model: "kimi-k2-turbo-preview",
        models: &["kimi-k2-turbo-preview"],
        source_url: "https://platform.moonshot.cn/docs/api/chat",
        verified: true,
    },
    SupplierDef {
        id: "zhipu",
        name: "智谱 GLM",
        vendor: "Zhipu",
        category: SupplierCategory::ModelVendor,
        website: "https://open.bigmodel.cn",
        base_url: "https://open.bigmodel.cn/api/paas/v4",
        protocol: GatewayProtocol::ChatCompletions,
        default_model: "glm-5",
        models: &["glm-5", "glm-5-flash"],
        source_url: "https://docs.bigmodel.cn/cn/guide/develop/http/introduction",
        verified: true,
    },
    SupplierDef {
        id: "qwen",
        name: "通义千问 / 阿里百炼",
        vendor: "Alibaba",
        category: SupplierCategory::ModelVendor,
        website: "https://bailian.console.aliyun.com",
        // 百炼的 OpenAI 兼容层
        base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1",
        protocol: GatewayProtocol::ChatCompletions,
        default_model: "qwen3-max",
        models: &["qwen3-max", "qwen3-coder-plus"],
        source_url: "https://help.aliyun.com/zh/model-studio/compatibility-of-openai-with-dashscope",
        verified: true,
    },
    SupplierDef {
        id: "volcengine",
        name: "火山引擎 Ark",
        vendor: "ByteDance",
        category: SupplierCategory::ModelVendor,
        website: "https://www.volcengine.com/product/ark",
        // 本机 Hermes 配置里是 /api/coding/v3（Coding 专用）；通用端点是 /api/v3
        base_url: "https://ark.cn-beijing.volces.com/api/v3",
        protocol: GatewayProtocol::ChatCompletions,
        default_model: "doubao-seed-1-6",
        models: &["doubao-seed-1-6"],
        source_url: "https://www.volcengine.com/docs/82379/1298454",
        verified: true,
    },
    SupplierDef {
        id: "mimo",
        name: "小米 MiMo",
        vendor: "Xiaomi",
        category: SupplierCategory::ModelVendor,
        website: "https://platform.xiaomimimo.com",
        // 本机 Hermes 配置实测值
        base_url: "https://api.xiaomimimo.com/v1",
        protocol: GatewayProtocol::ChatCompletions,
        default_model: "mimo-2.6-flash",
        models: &["mimo-2.6-flash"],
        source_url: "https://platform.xiaomimimo.com/",
        verified: true,
    },
    SupplierDef {
        id: "minimax",
        name: "MiniMax",
        vendor: "MiniMax",
        category: SupplierCategory::ModelVendor,
        website: "https://platform.minimaxi.com",
        base_url: "https://api.minimaxi.com/v1",
        protocol: GatewayProtocol::ChatCompletions,
        default_model: "MiniMax-M2",
        models: &["MiniMax-M2"],
        source_url: "https://platform.minimaxi.com/document",
        verified: true,
    },
    SupplierDef {
        id: "stepfun",
        name: "阶跃 StepFun",
        vendor: "StepFun",
        category: SupplierCategory::ModelVendor,
        website: "https://platform.stepfun.com",
        base_url: "https://api.stepfun.com/v1",
        protocol: GatewayProtocol::ChatCompletions,
        default_model: "step-3",
        models: &["step-3"],
        source_url: "https://platform.stepfun.com/docs/zh/api-reference",
        verified: true,
    },
    SupplierDef {
        id: "tencent",
        name: "腾讯云混元",
        vendor: "Tencent",
        category: SupplierCategory::ModelVendor,
        website: "https://cloud.tencent.com/product/hunyuan",
        base_url: "https://api.hunyuan.cloud.tencent.com/v1",
        protocol: GatewayProtocol::ChatCompletions,
        default_model: "hunyuan-turbos-latest",
        models: &["hunyuan-turbos-latest"],
        source_url: "https://cloud.tencent.com/document/product/1729/111007",
        verified: true,
    },
    SupplierDef {
        id: "openrouter",
        name: "OpenRouter",
        vendor: "OpenRouter",
        category: SupplierCategory::Aggregator,
        website: "https://openrouter.ai",
        // 项目里拉价格表用的就是 openrouter.ai/api/v1/models
        base_url: "https://openrouter.ai/api/v1",
        protocol: GatewayProtocol::ChatCompletions,
        default_model: "anthropic/claude-sonnet-4.5",
        models: &[],
        source_url: "https://openrouter.ai/docs/quickstart",
        verified: true,
    },
    // ---- 以下未能用官方文档确认地址：进目录但标「待核实」 ----
    SupplierDef {
        id: "qianfan",
        name: "百度千帆",
        vendor: "Baidu",
        category: SupplierCategory::ModelVendor,
        website: "https://cloud.baidu.com/product/qianfan",
        base_url: "https://qianfan.baidubce.com/v2",
        protocol: GatewayProtocol::ChatCompletions,
        default_model: "ernie-5.0",
        models: &[],
        source_url: "https://cloud.baidu.com/doc/qianfan-api",
        verified: false,
    },
    SupplierDef {
        id: "bailing",
        name: "蚂蚁百灵 Ling",
        vendor: "Ant Group",
        category: SupplierCategory::ModelVendor,
        website: "https://ling.tbox.cn",
        base_url: "https://api.tbox.cn/api/llm/v1",
        protocol: GatewayProtocol::ChatCompletions,
        default_model: "Ling-1T",
        models: &[],
        source_url: "https://alipaytbox.yuque.com/sxs0ba/ling/intro",
        verified: false,
    },
    SupplierDef {
        id: "longcat",
        name: "LongCat",
        vendor: "Meituan",
        category: SupplierCategory::ModelVendor,
        website: "https://longcat.chat",
        base_url: "https://api.longcat.chat/openai/v1",
        protocol: GatewayProtocol::ChatCompletions,
        default_model: "LongCat-Flash-Chat",
        models: &[],
        source_url: "https://longcat.chat/platform/docs",
        verified: false,
    },
    SupplierDef {
        id: "byteplus",
        name: "BytePlus ModelArk",
        vendor: "BytePlus",
        category: SupplierCategory::ModelVendor,
        website: "https://www.byteplus.com/product/modelark",
        base_url: "https://ark.ap-southeast.bytepluses.com/api/v3",
        protocol: GatewayProtocol::ChatCompletions,
        default_model: "seed-1-6-250615",
        models: &[],
        source_url: "https://docs.byteplus.com/en/docs/ModelArk",
        verified: false,
    },
    SupplierDef {
        id: "meta",
        name: "Meta Model API",
        vendor: "Meta",
        category: SupplierCategory::ModelVendor,
        website: "https://dev.meta.ai",
        base_url: "https://api.llama.com/compat/v1",
        protocol: GatewayProtocol::ChatCompletions,
        default_model: "Llama-4-Maverick",
        models: &[],
        source_url: "https://dev.meta.ai/docs",
        verified: false,
    },
    SupplierDef {
        id: "api88",
        name: "88API",
        vendor: "88API",
        category: SupplierCategory::Aggregator,
        website: "https://88api.ai",
        base_url: "https://api.88api.ai/v1",
        protocol: GatewayProtocol::ChatCompletions,
        default_model: "",
        models: &[],
        source_url: "https://88api.ai/",
        verified: false,
    },
    SupplierDef {
        id: "code9527",
        name: "9527CODE",
        vendor: "9527CODE",
        category: SupplierCategory::Aggregator,
        website: "https://9527.codes",
        base_url: "https://api.9527.codes/v1",
        protocol: GatewayProtocol::ChatCompletions,
        default_model: "",
        models: &[],
        source_url: "https://9527.codes/",
        verified: false,
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn ids_are_unique_and_lowercase() {
        let mut seen = BTreeSet::new();
        for supplier in SUPPLIERS {
            assert!(
                supplier
                    .id
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "id 必须是小写字母/数字/连字符：{}",
                supplier.id
            );
            assert!(seen.insert(supplier.id), "id 重复：{}", supplier.id);
        }
    }

    #[test]
    fn base_urls_are_https() {
        for supplier in SUPPLIERS {
            assert!(
                supplier.base_url.starts_with("https://"),
                "{} 的 base_url 必须是 https：{}",
                supplier.id,
                supplier.base_url
            );
            assert!(
                !supplier.base_url.ends_with('/'),
                "{} 的 base_url 不能以 / 结尾（拼接时会出双斜杠）",
                supplier.id
            );
        }
    }

    #[test]
    fn every_entry_has_source_and_unverified_are_flagged() {
        for supplier in SUPPLIERS {
            assert!(
                supplier.source_url.starts_with("https://"),
                "{} 必须给官方文档地址",
                supplier.id
            );
            // 未核实的条目数量应当可数：如果哪天全部 verified，这条断言会提醒我们
            // 把它删掉，而不是让「待核实」标记悄悄失去意义。
            let unverified = SUPPLIERS.iter().filter(|item| !item.verified).count();
            assert!(unverified > 0, "至少要保留明确的核实状态区分");
        }
    }

    #[test]
    fn default_model_belongs_to_models_when_models_are_listed() {
        for supplier in SUPPLIERS {
            if supplier.models.is_empty() || supplier.default_model.is_empty() {
                continue;
            }
            assert!(
                supplier.models.contains(&supplier.default_model),
                "{} 的默认模型不在预置清单里",
                supplier.id
            );
        }
    }
}
