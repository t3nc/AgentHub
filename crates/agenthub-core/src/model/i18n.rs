use serde::{Deserialize, Serialize};

/// Phase 5（翻译服务）只落地类型与本地缓存，不发起任何网络请求。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranslationKey {
    /// sha256(规范化原文 + lang + prompt_version)
    pub hash: String,
    pub lang: String,
}

impl TranslationKey {
    pub fn zh(hash: impl Into<String>) -> Self {
        Self {
            hash: hash.into(),
            lang: "zh-CN".into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranslationOrigin {
    /// 命中 ~/.agenthub 本地缓存
    LocalCache,
    /// 命中服务端公共缓存
    RemoteCache,
    /// 本次现场翻译
    Fresh,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationEntry {
    pub key: TranslationKey,
    pub translation: String,
    pub model: Option<String>,
    pub created_at: i64,
    pub origin: TranslationOrigin,
}

/// 逐条同意模型：默认不向服务端上传任何原文。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TranslationConsent {
    /// 全局开关，默认关闭
    pub enabled: bool,
    /// 永不上传的技能 / MCP 名称
    #[serde(default)]
    pub never_upload: Vec<String>,
    /// 已逐条同意过的名称
    #[serde(default)]
    pub allowed: Vec<String>,
    #[serde(default)]
    pub last_asked_at: Option<i64>,
}

impl TranslationConsent {
    pub fn can_upload(&self, name: &str) -> bool {
        self.enabled
            && !self.never_upload.iter().any(|n| n == name)
            && self.allowed.iter().any(|n| n == name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranslationStatus {
    /// 服务未启用（Phase 5 尚未接入）
    ServiceDisabled,
    /// 本地已有译文
    LocalHit,
    /// 需要用户逐条同意后才能上传
    ConsentRequired,
    /// 内容疑似敏感，需显式放行
    Sensitive,
}
