use crate::l10n::{Msg, t};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::model::i18n::{
    TranslationConsent, TranslationEntry, TranslationKey, TranslationOrigin, TranslationStatus,
};
use crate::paths;
use crate::store::Db;

/// Phase 5：技能 / MCP 介绍的中文翻译。
///
/// 服务端方案（Cloudflare Worker + D1 公共缓存）尚未实现，
/// 本期只落地本地缓存与 trait，不发任何网络请求。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationRequest {
    pub key: TranslationKey,
    pub name: String,
    pub kind: String,
    pub source_text: String,
}

pub trait Translator {
    fn service_enabled(&self) -> bool {
        false
    }

    fn query(&self, keys: &[TranslationKey]) -> Result<Vec<TranslationEntry>>;

    fn translate(&self, request: &TranslationRequest) -> Result<TranslationEntry>;
}

/// 未接入服务时的实现：读本地缓存，写一律报「服务未启用」。
pub struct LocalOnlyTranslator<'a> {
    pub db: &'a Db,
    pub consent: TranslationConsent,
}

impl<'a> Translator for LocalOnlyTranslator<'a> {
    fn query(&self, keys: &[TranslationKey]) -> Result<Vec<TranslationEntry>> {
        let mut out = Vec::new();
        for key in keys {
            if let Some(text) = self.db.get_translation(&key.hash, &key.lang)? {
                out.push(TranslationEntry {
                    key: key.clone(),
                    translation: text,
                    model: None,
                    created_at: paths::now_ts(),
                    origin: TranslationOrigin::LocalCache,
                });
            }
        }
        Ok(out)
    }

    fn translate(&self, _request: &TranslationRequest) -> Result<TranslationEntry> {
        Err(Error::Unsupported(t!(Msg::ErrTranslateServiceDisabled)))
    }
}

/// 判断某条内容当前可以走到哪一步，UI 用它决定按钮文案。
pub fn status(consent: &TranslationConsent, name: &str, has_local: bool) -> TranslationStatus {
    if has_local {
        return TranslationStatus::LocalHit;
    }
    if consent.never_upload.iter().any(|n| n == name) {
        return TranslationStatus::Sensitive;
    }
    if !consent.enabled || !consent.allowed.iter().any(|n| n == name) {
        return TranslationStatus::ConsentRequired;
    }
    TranslationStatus::ServiceDisabled
}

/// 上传前的敏感内容启发式判断：命中则默认不自动请求同意。
pub fn looks_sensitive(text: &str) -> bool {
    let lowered = text.to_ascii_lowercase();
    let markers = [
        "-----begin",
        "akia",
        "ghp_",
        "sk-",
        "password",
        "secret",
        "api_key",
        "apikey",
        "token=",
    ];
    markers.iter().any(|m| lowered.contains(m))
}
