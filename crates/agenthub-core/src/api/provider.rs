//! Phase 3 的读写入口：provider 登记、按 agent 绑定、生成写入计划。
//!
//! 分工与 MCP 一致：
//! - 这里只做**编排**（校验输入、解析密钥、拼 `ChangeSet`）；
//! - 合并文本是 `ext::provider` 的纯函数；
//! - 真正落盘一律走 `apply`（备份 + 批次 + 回滚）。

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::api::AgentHub;
use crate::config::AppConfig;
use crate::error::{Error, Result};
use crate::ext::provider::{self, ModelLimits, Provider, ProviderBinding, ProviderWrite};
use crate::l10n::{Msg, t};
use crate::mcp;
use crate::model::agent::{AgentId, Scope};
use crate::model::change::{ChangeKind, ChangeSet, FileOp, OpAction, RiskLevel};
use crate::model::gateway::{FeatureKey, GatewayProtocol, PoolMode};
use crate::model::mcp::SecretRef;
use crate::paths;

const MAX_CONFIG_BYTES: usize = 8 * 1024 * 1024;

/// 对外暴露的 provider（密钥一律脱敏）。
#[derive(Debug, Clone, Serialize)]
pub struct ProviderView {
    pub id: String,
    pub name: String,
    pub base_url: String,
    /// `env:VAR` / `keychain:svc/acct` / `literal:********` —— 永远不含明文
    pub api_key: String,
    pub models: Vec<String>,
    pub note: Option<String>,
    /// 上游协议；`None` = 未声明（按路由下游协议透传）
    pub protocol: Option<GatewayProtocol>,
    /// 每个模型的长度信息（只用于展示）
    pub model_limits: BTreeMap<String, ModelLimits>,
    /// 内置目录来源 id（自定义供应商为 None）
    pub catalog_id: Option<String>,
    /// 模型长度的摘要，形如「200K 上下文 · 64K 输出」；全没填时为 None
    pub limits_summary: Option<String>,
}

/// 某个 agent 当前的绑定状态（未绑定时 provider_id / model 为 None）。
#[derive(Debug, Clone, Serialize)]
pub struct BindingView {
    pub agent: String,
    pub display_name: String,
    /// 本轮是否接入了该 agent 的 provider 写入
    pub supported: bool,
    /// 不支持时的具体原因（格式未核实 / 结构不匹配），直接展示给用户
    pub unsupported_reason: Option<String>,
    pub provider_id: Option<String>,
    pub model: Option<String>,
    pub make_default: bool,
    pub via_gateway: bool,
    pub route_id: Option<String>,
    /// 三模式：直连 / 路由 / 聚合
    pub mode: PoolMode,
    /// 经网关时 agent 配置里会写的地址（统一入口 `/v1`）
    pub gateway_base_url: Option<String>,
    /// 经网关时的令牌 —— 就是 agent slug，网关靠它选中这条池
    pub gateway_token: Option<String>,
    /// Claude 四个模型槽位（sonnet/opus/haiku/fable → 上游模型名）
    pub model_slots: BTreeMap<String, String>,
    /// 池里已订阅的供应商 id（顺序即界面顺序）
    pub pool: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ProvidersView {
    pub providers: Vec<ProviderView>,
    pub bindings: Vec<BindingView>,
}

/// 新增 / 编辑 provider 的提交结构。
#[derive(Debug, Clone, Deserialize)]
pub struct ProviderInput {
    pub id: String,
    pub name: String,
    pub base_url: String,
    /// `env:VAR` / `keychain:svc/acct` / 明文（明文必须 `acknowledged = true`）
    pub api_key: String,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub note: Option<String>,
    /// 用明文密钥时的风险确认；不清空则后端拒绝
    #[serde(default)]
    pub acknowledged: bool,
    /// 上游协议；不传 = 未声明（沿用旧行为）
    #[serde(default)]
    pub protocol: Option<GatewayProtocol>,
    /// 每个模型的上下文 / 最大输出（只登记展示）
    #[serde(default)]
    pub model_limits: BTreeMap<String, ModelLimits>,
    /// 从内置目录选中的渠道 id
    #[serde(default)]
    pub catalog_id: Option<String>,
}

/// 探测模型列表的提交结构。
#[derive(Debug, Clone, Deserialize)]
pub struct ProbeInput {
    /// 供应商的 API 请求地址（通常已含 `/v1` 之类的版本段）
    pub base_url: String,
    /// `env:VAR` / `keychain:svc/acct` / 明文
    pub api_key: String,
    #[serde(default)]
    pub protocol: Option<GatewayProtocol>,
}

/// 探测到的单个模型。
#[derive(Debug, Clone, Serialize)]
pub struct ProbedModel {
    pub id: String,
    /// 上下文长度（token）。上游没给就是 None —— 不猜、不按 0 处理
    pub context: Option<u64>,
    /// 最大输出（token）
    pub max_output: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProbeResult {
    pub models: Vec<ProbedModel>,
    /// 实际请求的地址，界面上显示出来便于用户核对「是不是地址填错了」
    pub endpoint: String,
    /// 失败原因；成功为 None
    pub error: Option<String>,
}

/// 把某个 provider 绑到某个 agent 上。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct BindInput {
    pub agent: String,
    pub provider_id: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub make_default: bool,
    /// 旧字段：`true` = 经本地网关。新界面用 `mode`，这里保留给 CLI 与老调用。
    #[serde(default)]
    pub via_gateway: bool,
    /// 经网关时走哪条路由；缺省用 agent 自己的池路由（id = slug）
    #[serde(default)]
    pub route_id: Option<String>,
    /// 三模式；缺省按 `via_gateway` 推导
    #[serde(default)]
    pub mode: Option<PoolMode>,
    /// Claude 的模型槽位映射（其它 agent 忽略）
    #[serde(default)]
    pub model_slots: BTreeMap<String, String>,
}

/// 只读视图：注册表 + 各内置 agent 的绑定状态。
pub fn view(config: &AppConfig) -> ProvidersView {
    let providers = config
        .providers
        .iter()
        .map(|p| ProviderView {
            id: p.id.clone(),
            name: p.name.clone(),
            base_url: p.base_url.clone(),
            api_key: p.api_key.masked(),
            models: p.models.clone(),
            note: p.note.clone(),
            protocol: p.protocol,
            model_limits: p.model_limits.clone(),
            catalog_id: p.catalog_id.clone(),
            limits_summary: limits_summary(p),
        })
        .collect();

    let bindings = AgentId::builtin()
        .into_iter()
        .map(|agent| {
            let slug = agent.slug();
            let binding = config.provider_bindings.get(&slug);
            let mode = binding.map(|b| b.effective_mode()).unwrap_or_default();
            let via_gateway = mode.uses_gateway();
            let route_id = binding.and_then(|b| b.route_id.clone());
            // 池里已订阅的供应商（顺序即界面顺序）
            let pool = config
                .gateway_routes
                .iter()
                .find(|route| route.id == slug)
                .map(|route| {
                    route
                        .members
                        .iter()
                        .map(|member| member.provider_id.clone())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            BindingView {
                display_name: agent.display_name(),
                supported: provider::supports(&agent),
                unsupported_reason: if provider::supports(&agent) {
                    None
                } else {
                    Some(provider::unsupported_reason(&agent))
                },
                provider_id: binding.map(|b| b.provider_id.clone()),
                model: binding.map(|b| b.model.clone()),
                make_default: binding.map(|b| b.make_default).unwrap_or(false),
                // 网关模式统一走 /v1，老形态的 /gw/<route> 只在池 id 不是 slug 时出现
                gateway_base_url: if via_gateway {
                    Some(crate::ext::gateway::unified_base_url(config.gateway.port))
                } else {
                    None
                },
                gateway_token: if via_gateway {
                    Some(slug.clone())
                } else {
                    None
                },
                via_gateway,
                route_id,
                mode,
                model_slots: binding.map(|b| b.model_slots.clone()).unwrap_or_default(),
                pool,
                agent: slug,
            }
        })
        .collect();

    ProvidersView {
        providers,
        bindings,
    }
}

/// 校验输入并构造 `Provider`（纯函数，便于单测）。
pub fn build_provider(input: &ProviderInput) -> Result<Provider> {
    let id = input.id.trim().to_ascii_lowercase();
    if id.is_empty() {
        return Err(Error::msg(t!(Msg::ErrProviderIdEmpty)));
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(Error::msg(t!(Msg::ErrProviderIdChars, id = id)));
    }

    let name = input.name.trim();
    if name.is_empty() {
        return Err(Error::msg(t!(Msg::ErrProviderNameEmpty)));
    }

    let base_url = input.base_url.trim();
    if base_url.is_empty() {
        return Err(Error::msg(t!(Msg::ErrBaseUrlEmpty)));
    }
    if !(base_url.starts_with("http://") || base_url.starts_with("https://")) {
        return Err(Error::msg(t!(Msg::ErrBaseUrlScheme)));
    }

    let raw_key = input.api_key.trim();
    if raw_key.is_empty() {
        return Err(Error::msg(t!(Msg::ErrApiKeyEmpty)));
    }
    let api_key = match SecretRef::parse(raw_key) {
        SecretRef::Literal { value, .. } => {
            if !input.acknowledged {
                return Err(Error::msg(t!(Msg::ErrLiteralKeyUnacknowledged)));
            }
            SecretRef::Literal {
                value,
                acknowledged: true,
            }
        }
        SecretRef::Env { var } if var.trim().is_empty() => {
            return Err(Error::msg(t!(Msg::ErrEnvVarEmpty)));
        }
        SecretRef::Keychain { service, .. } if service.trim().is_empty() => {
            return Err(Error::msg(t!(Msg::ErrKeychainFormat)));
        }
        other => other,
    };

    let mut models: Vec<String> = Vec::new();
    for raw in &input.models {
        let model = raw.trim();
        if model.is_empty() {
            continue;
        }
        if !models.iter().any(|m| m == model) {
            models.push(model.to_string());
        }
    }

    let note = input
        .note
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(str::to_string);

    // 只保留仍然存在的模型的长度信息：模型被删掉后不该留一条孤立记录
    let model_limits: BTreeMap<String, ModelLimits> = input
        .model_limits
        .iter()
        .filter(|(model, _)| models.iter().any(|m| m == *model))
        .map(|(model, limits)| (model.clone(), limits.clone()))
        .collect();

    Ok(Provider {
        id,
        name: name.to_string(),
        base_url: base_url.to_string(),
        api_key,
        models,
        note,
        protocol: input.protocol,
        model_limits,
        catalog_id: input
            .catalog_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(str::to_string),
    })
}

/// 探测时最多读 2MB：模型列表正常几十 KB，超过说明地址指向了别的东西。
const PROBE_MAX_BYTES: usize = 2 * 1024 * 1024;

/// 从供应商的 `/models` 拉模型列表。
///
/// 只在用户点「获取模型列表」时调用（会联网）。失败不进 `Result` 而是塞进
/// `ProbeResult.error`：探测失败不该中断保存，界面只要一行提示。
pub fn probe_models(input: &ProbeInput) -> ProbeResult {
    probe_models_with(&crate::market::http::UreqHttp::new(), input)
}

/// 可注入 HTTP 实现的版本（离线测试用）。
pub fn probe_models_with(http: &dyn crate::market::http::Http, input: &ProbeInput) -> ProbeResult {
    let endpoint = models_endpoint(&input.base_url);
    let mut result = ProbeResult {
        models: Vec::new(),
        endpoint: endpoint.clone(),
        error: None,
    };

    if !(endpoint.starts_with("http://") || endpoint.starts_with("https://")) {
        result.error = Some(t!(Msg::ProbeErrScheme));
        return result;
    }

    let secret = SecretRef::parse(&input.api_key);
    let key = match resolve_for_probe(&secret) {
        Ok(value) => value,
        Err(err) => {
            result.error = Some(t!(Msg::ProbeErrSecret, error = err));
            return result;
        }
    };

    // Anthropic 系走 x-api-key + 版本头，其余走 Bearer
    let bearer = format!("Bearer {key}");
    let headers: Vec<(&str, &str)> = match input.protocol {
        Some(GatewayProtocol::Anthropic) => vec![
            ("x-api-key", key.as_str()),
            ("anthropic-version", "2023-06-01"),
        ],
        _ => vec![("authorization", bearer.as_str())],
    };

    let response = match http.get_with(&endpoint, &headers, PROBE_MAX_BYTES) {
        Ok(response) => response,
        Err(err) => {
            result.error = Some(t!(Msg::ProbeErrRequest, error = err));
            return result;
        }
    };

    if !response.ok() {
        result.error = Some(match response.status {
            401 | 403 => t!(Msg::ProbeErrAuth),
            404 => t!(Msg::ProbeErrNotFound),
            429 => t!(Msg::ProbeErrRateLimit),
            code => t!(Msg::ProbeErrStatus, code = code),
        });
        return result;
    }

    let text = match response.text() {
        Ok(text) => text,
        Err(err) => {
            result.error = Some(err.to_string());
            return result;
        }
    };
    let value: serde_json::Value = match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(_) => {
            result.error = Some(t!(Msg::ProbeErrNotJson));
            return result;
        }
    };

    // 兼容 { data: [...] }（OpenAI / Anthropic）、{ models: [...] }（部分国产兼容层）与裸数组
    let entries = value
        .get("data")
        .and_then(serde_json::Value::as_array)
        .or_else(|| value.get("models").and_then(serde_json::Value::as_array))
        .or_else(|| value.as_array());
    let Some(entries) = entries else {
        result.error = Some(t!(Msg::ProbeErrNoModels));
        return result;
    };

    let mut seen: BTreeSet<String> = BTreeSet::new();
    for entry in entries {
        let id = entry
            .get("id")
            .and_then(serde_json::Value::as_str)
            .or_else(|| entry.get("name").and_then(serde_json::Value::as_str))
            .map(str::trim)
            .filter(|id| !id.is_empty());
        let Some(id) = id else { continue };
        if !seen.insert(id.to_string()) {
            continue;
        }
        result.models.push(ProbedModel {
            id: id.to_string(),
            context: pick_u64(
                entry,
                &[
                    "context_length",
                    "max_context_length",
                    "max_input_tokens",
                    "context_window",
                    "input_token_limit",
                ],
            ),
            max_output: pick_u64(
                entry,
                &[
                    "max_output_tokens",
                    "max_completion_tokens",
                    "output_token_limit",
                ],
            )
            .or_else(|| {
                entry
                    .get("top_provider")
                    .and_then(|provider| provider.get("max_completion_tokens"))
                    .and_then(serde_json::Value::as_u64)
            }),
        });
    }
    result.models.sort_by(|a, b| a.id.cmp(&b.id));

    if result.models.is_empty() {
        result.error = Some(t!(Msg::ProbeErrEmptyModels));
    }
    result
}

/// 探测用的密钥解析。
///
/// 与 `mcp::secrets::resolve` 的唯一差别：明文不要求 `acknowledged`。
/// 那个确认是「**会写进 agent 配置文件**」的风险提示，而探测只发一次只读请求、
/// 不落任何盘；在表单里刚填完 key 就点「获取模型列表」是主路径，不该被拦住。
fn resolve_for_probe(secret: &SecretRef) -> Result<String> {
    match secret {
        SecretRef::Literal { value, .. } => Ok(value.clone()),
        other => mcp::secrets::resolve(other),
    }
}

/// `{base_url}/models`。`base_url` 通常已经带版本段（`/v1`、`/v3`、`/v4`），
/// 所以这里只做尾斜杠归一与幂等判断，不再自作主张补 `/v1`。
fn models_endpoint(base_url: &str) -> String {
    let base = base_url.trim().trim_end_matches('/');
    if base.is_empty() || base.ends_with("/models") {
        return base.to_string();
    }
    format!("{base}/models")
}

/// 从若干候选键里取第一个能当 u64 的值（有些上游把数字写成字符串）。
fn pick_u64(entry: &serde_json::Value, keys: &[&str]) -> Option<u64> {
    for key in keys {
        let Some(value) = entry.get(*key) else {
            continue;
        };
        if let Some(number) = value.as_u64() {
            return Some(number);
        }
        if let Some(text) = value.as_str()
            && let Ok(number) = text.trim().parse::<u64>()
        {
            return Some(number);
        }
    }
    None
}

/// 供应商级别的长度摘要：取第一个配了长度的模型作为代表。
///
/// 一个供应商下各模型长度可能不同，列表里只显示一个「代表值」，
/// 具体到某个模型看详情或绑定处的下拉即可。
fn limits_summary(provider: &Provider) -> Option<String> {
    provider
        .models
        .iter()
        .filter_map(|model| provider.model_limits.get(model))
        .find(|limits| !limits.is_empty())
        .and_then(ModelLimits::display)
}

impl AgentHub {
    /// 只读视图。功能被禁用时仍可读（设置页需要它渲染「受影响的 agent」提示），
    /// 但界面上已经没有入口；写入与生成计划另有门禁。
    pub fn providers_view(&self) -> ProvidersView {
        view(&self.config)
    }

    /// 新增或覆盖一条 provider（只写 `config.json`，不碰任何 agent 配置）。
    pub fn upsert_provider(&mut self, input: ProviderInput) -> Result<ProvidersView> {
        self.require_feature(FeatureKey::Provider)?;
        let provider = build_provider(&input)?;
        let id = provider.id.clone();
        match self.config.providers.iter_mut().find(|p| p.id == id) {
            Some(slot) => *slot = provider,
            None => self.config.providers.push(provider),
        }
        self.config.providers.sort_by(|a, b| a.id.cmp(&b.id));

        // provider 的模型列表变了以后，已绑定的 model 可能不再存在：收敛到第一个模型
        let models = self
            .config
            .providers
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.models.clone())
            .unwrap_or_default();
        if !models.is_empty() {
            for binding in self.config.provider_bindings.values_mut() {
                if binding.provider_id == id && !models.contains(&binding.model) {
                    binding.model = models[0].clone();
                }
            }
        }

        self.config.save()?;
        Ok(self.providers_view())
    }

    /// 删除 provider，并清掉指向它的绑定（避免留下悬空引用）。
    pub fn delete_provider(&mut self, id: &str) -> Result<ProvidersView> {
        self.require_feature(FeatureKey::Provider)?;
        let before = self.config.providers.len();
        self.config.providers.retain(|p| p.id != id);
        self.config
            .provider_bindings
            .retain(|_, binding| binding.provider_id != id);
        if self.config.providers.len() != before {
            self.config.save()?;
        }
        Ok(self.providers_view())
    }

    /// 绑定：把 provider 与模型记到某个 agent 上。
    pub fn bind_provider(&mut self, input: BindInput) -> Result<ProvidersView> {
        self.require_feature(FeatureKey::Provider)?;
        let agent = AgentId::from_slug(input.agent.trim());
        if !provider::supports(&agent) {
            return Err(Error::Unsupported(provider::unsupported_reason(&agent)));
        }
        let found = self
            .config
            .providers
            .iter()
            .find(|p| p.id == input.provider_id.trim())
            .ok_or_else(|| Error::NotFound(t!(Msg::ErrProviderMissing, id = input.provider_id)))?;

        // 只写变量名的格式（Codex 的 env_key）在绑定时就拒绝 keychain / 明文。
        // 否则会先落一条永远无法应用的绑定，config.json 里留下悬空状态（plan 阶段才报错太晚）。
        if !provider::key_is_literal(&agent) && !matches!(found.api_key, SecretRef::Env { .. }) {
            return Err(Error::Unsupported(t!(
                Msg::ErrEnvKeyOnlyBind,
                agent = agent.display_name(),
                masked = found.api_key.masked()
            )));
        }

        let model = match input
            .model
            .as_deref()
            .map(str::trim)
            .filter(|m| !m.is_empty())
        {
            Some(model) => {
                if !found.models.is_empty() && !found.models.iter().any(|m| m == model) {
                    return Err(Error::msg(t!(
                        Msg::ErrModelNotInProvider,
                        model = model,
                        name = found.name,
                        models = found.models.join(" / ")
                    )));
                }
                model.to_string()
            }
            None => found
                .models
                .first()
                .cloned()
                .ok_or_else(|| Error::msg(t!(Msg::ErrProviderNoModels, name = found.name)))?,
        };

        // 三模式：新界面直接给 mode；老调用只给 via_gateway，按「经网关 = 路由」推。
        let mode = input.mode.unwrap_or(if input.via_gateway {
            PoolMode::Route
        } else {
            PoolMode::Direct
        });

        // 直连时路由不参与。留着 route_id 只会让界面/报告误以为「经网关生效了」
        // （现实数据里就出现过 `via_gateway: false` + `route_id: "agnescn"` 的组合）。
        // 网关模式下没指定就用 agent 自己的池路由（id = slug）。
        let route_id = if mode.uses_gateway() {
            Some(
                input
                    .route_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|id| !id.is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(|| agent.slug()),
            )
        } else {
            None
        };

        // 这个 agent 能不能吃这种协议：Codex 接不了 Anthropic，Claude 直连接不了 Chat。
        // 挡在绑定之前，别留一条永远跑不通的配置。
        provider::agent_accepts_protocol(&agent, found.protocol, input.via_gateway)?;

        // 经网关时再把路由与上游的兼容性也挡在绑定之前
        if input.via_gateway
            && let Some(route) = route_id
                .as_deref()
                .and_then(|id| self.config.gateway_routes.iter().find(|r| r.id == id))
        {
            let upstream = crate::ext::gateway::upstream_protocol(route.protocol, found.protocol);
            if let crate::ext::gateway::Compatibility::Unsupported(reason) =
                crate::ext::gateway::protocol_compatible(route.protocol, upstream)
            {
                return Err(Error::Unsupported(t!(
                    Msg::ErrSupplierReason,
                    name = found.name,
                    reason = reason
                )));
            }
        }

        self.config.provider_bindings.insert(
            agent.slug(),
            ProviderBinding {
                agent,
                provider_id: found.id.clone(),
                model,
                make_default: input.make_default,
                // via_gateway 保留为派生值：老代码读它判断是否经网关
                via_gateway: mode.uses_gateway(),
                route_id,
                mode,
                model_slots: input.model_slots.clone(),
            },
        );
        self.config.save()?;
        Ok(self.providers_view())
    }

    pub fn unbind_provider(&mut self, agent_slug: &str) -> Result<ProvidersView> {
        self.require_feature(FeatureKey::Provider)?;
        let agent = AgentId::from_slug(agent_slug.trim());
        if self
            .config
            .provider_bindings
            .remove(&agent.slug())
            .is_some()
        {
            self.config.save()?;
        }
        Ok(self.providers_view())
    }

    /// 按已保存的绑定生成写入计划。
    ///
    /// 密钥在此刻解析：Claude Code 只能写真值，Codex 只写 `env_key` 变量名。
    /// 任何一步失败都直接返回错误，磁盘零改动。
    pub fn plan_provider_apply(&self, agent_slug: &str) -> Result<ChangeSet> {
        self.require_feature(FeatureKey::Provider)?;
        let agent = AgentId::from_slug(agent_slug.trim());
        if !provider::supports(&agent) {
            return Err(Error::Unsupported(provider::unsupported_reason(&agent)));
        }
        let slug = agent.slug();
        let binding = self
            .config
            .provider_bindings
            .get(&slug)
            .ok_or_else(|| Error::msg(t!(Msg::ErrAgentNoBinding, agent = agent.display_name())))?
            .clone();
        self.plan_for(&agent, &binding)
    }

    /// 按给定绑定生成计划。
    ///
    /// 抽出来是为了「回退到直连」复用同一条路径：那件事不需要先改写 `config.json`
    /// 里的绑定，只要用一份 `via_gateway = false` 的临时绑定算出计划即可。
    pub(crate) fn plan_for(&self, agent: &AgentId, binding: &ProviderBinding) -> Result<ChangeSet> {
        let agent = agent.clone();
        let chosen_origin = self
            .config
            .providers
            .iter()
            .find(|p| p.id == binding.provider_id)
            .ok_or_else(|| {
                Error::NotFound(t!(Msg::ErrProviderMissing, id = binding.provider_id))
            })?;

        // 经网关时，写进 agent 配置的是「本地地址 + 网关令牌」，
        // 上游密钥完全不出现。用一个副本承载这次写入的差异。
        let mode = binding.effective_mode();
        let mut effective = chosen_origin.clone();
        let mut key_override: Option<String> = None;
        let mut via_gateway_note: Option<String> = None;
        // 直连写上游地址；网关模式所有 agent 共用同一个 /v1 入口
        let mut base_url = effective.base_url.clone();
        if mode.uses_gateway() {
            let route_id = binding
                .route_id
                .as_deref()
                .map(str::trim)
                .filter(|id| !id.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| agent.slug());
            let route = self
                .config
                .gateway_routes
                .iter()
                .find(|r| r.id == route_id)
                .ok_or_else(|| Error::NotFound(t!(Msg::ErrPoolMissing, route = route_id)))?;
            if !route.enabled {
                return Err(Error::Unsupported(t!(
                    Msg::ErrPoolDisabled,
                    route = route_id
                )));
            }
            base_url = crate::ext::gateway::unified_base_url(self.config.gateway.port);
            // 令牌就是 agent 名：网关靠它选中这条池
            let token = agent.slug().to_string();
            if provider::key_is_literal(&agent) {
                key_override = Some(token);
            } else {
                // 只写变量名的格式：把 env_key 指向网关令牌变量
                effective.api_key = SecretRef::Env {
                    var: crate::api::gateway::GATEWAY_TOKEN_ENV.to_string(),
                };
            }
            via_gateway_note = Some(t!(
                Msg::PlanProviderViaGateway,
                url = base_url,
                token = agent.slug()
            ));
        }
        let chosen = &effective;
        // 旧配置里可能存在矩阵上线前就存在的组合，生成计划时再挡一次
        provider::agent_accepts_protocol(&agent, chosen.protocol, mode.uses_gateway())?;

        let config_dir = self.adapter(&agent).and_then(|a| a.config_dir());
        let spec = provider::spec_for(&agent, config_dir.as_deref())
            .ok_or_else(|| Error::Unsupported(provider::unsupported_reason(&agent)))?;
        let target = spec.file.clone();
        let existing = if target.exists() {
            Some(paths::read_to_string_limited(&target, MAX_CONFIG_BYTES)?)
        } else {
            None
        };

        // 写变量名的格式只写引用；写真值的格式在 plan 阶段解析密钥（失败即整体中止）。
        // 经网关时密钥换成网关令牌，上游密钥一次都不解析。
        let api_key_value = if let Some(token) = key_override {
            Some(token)
        } else if spec.key_is_literal {
            Some(mcp::secrets::resolve(&chosen.api_key)?)
        } else {
            match &chosen.api_key {
                SecretRef::Env { .. } => None,
                other => {
                    return Err(Error::Unsupported(t!(
                        Msg::ErrEnvKeyOnlyPlan,
                        agent = agent.display_name(),
                        masked = other.masked()
                    )));
                }
            }
        };

        let write = ProviderWrite {
            provider: chosen,
            model: &binding.model,
            make_default: binding.make_default,
            api_key_value: api_key_value.as_deref(),
            base_url: base_url.as_str(),
            mode,
            slots: &binding.model_slots,
        };
        let merged = provider::merge_for(&spec, existing.as_deref(), &write)?;

        let mut cs = ChangeSet::new(
            ChangeKind::Provider,
            t!(
                Msg::PlanTitleProvider,
                name = chosen.name,
                agent = agent.display_name()
            ),
        );
        cs.note(if existing.is_some() {
            t!(
                Msg::PlanProviderTargetExisting,
                at = paths::path_str(&target)
            )
        } else {
            t!(Msg::PlanProviderTargetCreate, at = paths::path_str(&target))
        });
        if let Some(note) = via_gateway_note {
            cs.note(note);
        }
        cs.note(match spec.kind {
            provider::TargetKind::ClaudeSettings => t!(Msg::PlanProviderClaudeNote),
            provider::TargetKind::CodexToml => t!(
                Msg::PlanProviderCodexNote,
                id = chosen.id,
                wire = provider::wire_api_for(chosen.protocol)
            ),
            provider::TargetKind::OpencodeJson => t!(
                Msg::PlanProviderOpencodeNote,
                id = chosen.id,
                npm = provider::npm_for(chosen.protocol)
            ),
            provider::TargetKind::JsonKeys => t!(
                Msg::PlanProviderJsonKeysNote,
                base = spec.base_url_key.replace("{id}", &chosen.id),
                key = spec.api_key_key.replace("{id}", &chosen.id)
            ),
        });

        if existing.as_deref() == Some(merged.as_str()) {
            cs.note(t!(Msg::PlanProviderUpToDate));
            return Ok(cs);
        }
        if matches!(chosen.api_key, SecretRef::Literal { .. }) {
            cs.risk(RiskLevel::Warn, t!(Msg::RiskProviderLiteralKey));
        }
        if binding.make_default {
            cs.risk(
                RiskLevel::Warn,
                t!(
                    Msg::RiskProviderDefaultModel,
                    model = binding.model,
                    agent = agent.display_name()
                ),
            );
        }
        cs.push(FileOp::write_file(target, merged, true));
        Ok(cs)
    }
}

// ---- 一键应用（切换器点一下走的就是这条链） ----

/// 写入后的一次连通性探测。
#[derive(Debug, Clone, Serialize)]
pub struct ProbeReport {
    pub endpoint: String,
    pub ok: bool,
    /// 一句话结论：「连接正常 · 12 个模型」/「密钥无效或没有权限（HTTP 401/403）」
    pub detail: String,
    pub models: usize,
}

/// 「点一下就写入」的完整结果。
#[derive(Debug, Clone, Serialize)]
pub struct ApplyReport {
    /// 是否真的写了盘（配置已是最新时为 false）
    pub applied: bool,
    /// 没写盘时的原因
    pub skipped: Option<String>,
    /// 涉及的文件（绝对路径）
    pub files: Vec<String>,
    /// 可用于回滚的批次号
    pub batch_id: Option<String>,
    pub probe: Option<ProbeReport>,
    pub notes: Vec<String>,
}

impl AgentHub {
    /// 把一个 agent 的当前绑定写进它的配置文件，然后探测一次连通性。
    ///
    /// **探测失败不回滚写入**：配置是对的，问题在上游或密钥。界面必须能区分
    /// 「写入失败」与「已写入但连不通」，否则用户会反复重试一个本来就写对了的配置。
    pub fn apply_provider_binding(&self, agent_slug: &str) -> Result<ApplyReport> {
        self.require_feature(FeatureKey::Provider)?;
        let agent = AgentId::from_slug(agent_slug.trim());
        let slug = agent.slug();
        let binding = self
            .config
            .provider_bindings
            .get(&slug)
            .cloned()
            .ok_or_else(|| {
                Error::NotFound(t!(Msg::ErrAgentNoSupplier, agent = agent.display_name()))
            })?;

        let plan = self.plan_for(&agent, &binding)?;
        let files: Vec<String> = plan
            .ops
            .iter()
            .filter_map(|op| match &op.action {
                OpAction::WriteFile { at, .. } => Some(at.display().to_string()),
                _ => None,
            })
            .collect();
        let mut notes = plan.notes.clone();

        let (applied, skipped, batch_id) = if plan.ops.is_empty() {
            (false, Some(t!(Msg::PlanProviderUpToDate)), None)
        } else {
            let result = self.apply(&plan, &Scope::User)?;
            notes.extend(result.log);
            (true, None, Some(result.batch_id))
        };

        let probe = self.probe_binding(&binding);
        Ok(ApplyReport {
            applied,
            skipped,
            files,
            batch_id,
            probe,
            notes,
        })
    }

    // ---- 供应商池：加入 / 移除 / 设为默认 / 切换模式 ----
    //
    // 「池」不新增数据结构：每个 agent 一条 id = slug 的 route，
    // `members` 就是界面上的「已添加」，`binding.provider_id` 就是「默认」。
    // 四个动作都以 `apply_provider_binding` 收尾，保证「点完就生效」。

    /// 把供应商加进该 agent 的池；池原本为空时自动设为默认。
    pub fn add_to_pool(&mut self, agent_slug: &str, provider_id: &str) -> Result<ApplyReport> {
        self.require_feature(FeatureKey::Provider)?;
        let agent = self.pool_agent(agent_slug)?;
        let slug = agent.slug();
        let provider = self
            .config
            .providers
            .iter()
            .find(|item| item.id == provider_id.trim())
            .ok_or_else(|| Error::NotFound(t!(Msg::ErrSupplierMissing, id = provider_id)))?
            .clone();

        let mode = self
            .config
            .provider_bindings
            .get(&slug)
            .map(|binding| binding.effective_mode())
            .unwrap_or_default();
        provider::agent_accepts_protocol(&agent, provider.protocol, mode.uses_gateway())?;

        self.ensure_pool_route(&agent, mode)?;
        let model = provider.models.first().cloned().unwrap_or_default();
        let route = self
            .pool_route_mut(&agent)
            .ok_or_else(|| Error::msg(t!(Msg::ErrPoolInit)))?;
        if !route.members.iter().any(|m| m.provider_id == provider.id) {
            route.members.push(crate::model::gateway::RouteMember {
                provider_id: provider.id.clone(),
                weight: 1,
                priority: route.members.len() as u32 + 1,
                models: Vec::new(),
                model_map: BTreeMap::new(),
                enabled: true,
            });
        }

        // 没有绑定、或原来绑的供应商已被删掉 → 这一位就是默认
        let needs_default = self
            .config
            .provider_bindings
            .get(&slug)
            .map(|binding| binding.provider_id.trim().is_empty())
            .unwrap_or(true);
        if needs_default {
            self.write_binding(&agent, &provider.id, &model, mode);
        }
        self.sync_alias_map(&agent, mode)?;
        self.config.save()?;
        self.apply_provider_binding(&slug)
    }

    /// 从池里移除；如果删掉的正是默认，把剩余第一个顶上。
    pub fn remove_from_pool(&mut self, agent_slug: &str, provider_id: &str) -> Result<ApplyReport> {
        self.require_feature(FeatureKey::Provider)?;
        let agent = self.pool_agent(agent_slug)?;
        let slug = agent.slug();
        let mode = self
            .config
            .provider_bindings
            .get(&slug)
            .map(|binding| binding.effective_mode())
            .unwrap_or_default();

        let mut rest: Vec<String> = Vec::new();
        if let Some(route) = self.pool_route_mut(&agent) {
            route
                .members
                .retain(|member| member.provider_id != provider_id.trim());
            rest = route
                .members
                .iter()
                .map(|m| m.provider_id.clone())
                .collect();
        }

        let was_default = self
            .config
            .provider_bindings
            .get(&slug)
            .map(|binding| binding.provider_id == provider_id.trim())
            .unwrap_or(false);
        if was_default {
            match rest.first() {
                Some(next) => {
                    let model = self.model_of(next);
                    self.write_binding(&agent, next, &model, mode);
                }
                // 池空了：绑定也清掉，别留一条指向已删供应商的悬空记录。
                // 这时没有可写入的绑定，直接返回而不是走 apply（否则会报「没有选择供应商」）。
                None => {
                    self.config.provider_bindings.remove(&slug);
                    self.config.save()?;
                    return Ok(ApplyReport {
                        applied: true,
                        skipped: None,
                        files: Vec::new(),
                        batch_id: None,
                        probe: None,
                        notes: vec![t!(Msg::NotePoolCleared, agent = agent.display_name())],
                    });
                }
            }
        }
        self.sync_alias_map(&agent, mode)?;
        self.config.save()?;
        self.apply_provider_binding(&slug)
    }

    /// 设为默认供应商（同时保证它在池里）。
    pub fn set_default_provider(
        &mut self,
        agent_slug: &str,
        provider_id: &str,
    ) -> Result<ApplyReport> {
        self.require_feature(FeatureKey::Provider)?;
        let agent = self.pool_agent(agent_slug)?;
        let slug = agent.slug();
        let provider = self
            .config
            .providers
            .iter()
            .find(|item| item.id == provider_id.trim())
            .ok_or_else(|| Error::NotFound(t!(Msg::ErrSupplierMissing, id = provider_id)))?
            .clone();
        let mode = self
            .config
            .provider_bindings
            .get(&slug)
            .map(|binding| binding.effective_mode())
            .unwrap_or_default();
        provider::agent_accepts_protocol(&agent, provider.protocol, mode.uses_gateway())?;

        self.ensure_pool_route(&agent, mode)?;
        let route = self
            .pool_route_mut(&agent)
            .ok_or_else(|| Error::msg(t!(Msg::ErrPoolInit)))?;
        if !route.members.iter().any(|m| m.provider_id == provider.id) {
            route.members.push(crate::model::gateway::RouteMember {
                provider_id: provider.id.clone(),
                weight: 1,
                priority: route.members.len() as u32 + 1,
                models: Vec::new(),
                model_map: BTreeMap::new(),
                enabled: true,
            });
        }

        // 沿用当前模型；新供应商没有这个模型就退回它自己的第一个
        let model = self
            .config
            .provider_bindings
            .get(&slug)
            .map(|binding| binding.model.clone())
            .filter(|model| provider.models.is_empty() || provider.models.contains(model))
            .unwrap_or_else(|| provider.models.first().cloned().unwrap_or_default());
        self.write_binding(&agent, &provider.id, &model, mode);
        self.sync_alias_map(&agent, mode)?;
        self.config.save()?;
        self.apply_provider_binding(&slug)
    }

    /// 切三模式。`Direct` 时把绑定退回直连但保留池，切回来不用重新加。
    pub fn set_pool_mode(&mut self, agent_slug: &str, mode: PoolMode) -> Result<ApplyReport> {
        self.require_feature(FeatureKey::Provider)?;
        let agent = self.pool_agent(agent_slug)?;
        let slug = agent.slug();

        // 切到网关前先把协议矩阵挡一遍
        if let Some(provider_id) = self
            .config
            .provider_bindings
            .get(&slug)
            .map(|binding| binding.provider_id.clone())
            && let Some(provider) = self.config.providers.iter().find(|p| p.id == provider_id)
        {
            provider::agent_accepts_protocol(&agent, provider.protocol, mode.uses_gateway())?;
        }

        if let Some(binding) = self.config.provider_bindings.get_mut(&slug) {
            binding.mode = mode;
            binding.via_gateway = mode.uses_gateway();
            binding.route_id = if mode.uses_gateway() {
                Some(slug.clone())
            } else {
                None
            };
        }
        self.ensure_pool_route(&agent, mode)?;
        if let Some(route) = self.pool_route_mut(&agent) {
            route.mode = mode.route_mode();
            route.enabled = mode.uses_gateway();
        }
        self.sync_alias_map(&agent, mode)?;
        self.config.save()?;
        self.apply_provider_binding(&slug)
    }

    /// 只更新 Claude 的模型槽位映射，其它字段不动。
    ///
    /// 单独一个方法而不是复用 `bind_provider`：改映射不该触碰「默认供应商 / 模型」，
    /// 否则一次误传就把用户的绑定改掉了。
    pub fn set_model_slots(
        &mut self,
        agent_slug: &str,
        slots: BTreeMap<String, String>,
    ) -> Result<ApplyReport> {
        self.require_feature(FeatureKey::Provider)?;
        let agent = self.pool_agent(agent_slug)?;
        if !matches!(agent, AgentId::Claude) {
            return Err(Error::Unsupported(t!(Msg::ErrSlotsClaudeOnly)));
        }
        let slug = agent.slug();
        let binding = self
            .config
            .provider_bindings
            .get(&slug)
            .cloned()
            .ok_or_else(|| Error::NotFound(t!(Msg::ErrClaudeNoSupplier)))?;
        let provider = self
            .config
            .providers
            .iter()
            .find(|item| item.id == binding.provider_id)
            .cloned()
            .ok_or_else(|| {
                Error::NotFound(t!(Msg::ErrSupplierMissing, id = binding.provider_id))
            })?;

        // 白名单 + 值必须在供应商的模型列表里（列表为空时跳过存在性检查）
        for (slot, value) in &slots {
            if !provider::SLOTS.contains(&slot.as_str()) {
                return Err(Error::Unsupported(t!(
                    Msg::ErrUnknownSlot,
                    slot = slot,
                    slots = provider::SLOTS.join(" / ")
                )));
            }
            let value = value.trim();
            if value.is_empty() {
                return Err(Error::Unsupported(t!(Msg::ErrSlotEmpty, slot = slot)));
            }
            if !provider.models.is_empty() && !provider.models.iter().any(|m| m == value) {
                return Err(Error::Unsupported(t!(
                    Msg::ErrSlotModelMissing,
                    slot = slot,
                    model = value,
                    name = provider.name
                )));
            }
        }

        if let Some(entry) = self.config.provider_bindings.get_mut(&slug) {
            entry.model_slots = slots
                .into_iter()
                .filter(|(_, value)| !value.trim().is_empty())
                .map(|(slot, value)| (slot, value.trim().to_string()))
                .collect();
        }
        let mode = self
            .config
            .provider_bindings
            .get(&slug)
            .map(|entry| entry.effective_mode())
            .unwrap_or_default();
        self.sync_alias_map(&agent, mode)?;
        self.config.save()?;
        self.apply_provider_binding(&slug)
    }

    /// 四个池动作共用的入口校验。
    fn pool_agent(&self, agent_slug: &str) -> Result<AgentId> {
        let agent = AgentId::from_slug(agent_slug.trim());
        if !provider::supports(&agent) {
            return Err(Error::Unsupported(provider::unsupported_reason(&agent)));
        }
        Ok(agent)
    }

    fn pool_route_mut(
        &mut self,
        agent: &AgentId,
    ) -> Option<&mut crate::model::gateway::GatewayRoute> {
        let slug = agent.slug();
        self.config
            .gateway_routes
            .iter_mut()
            .find(|route| route.id == slug)
    }

    /// 该 agent 的池路由不存在就建一条空的（成员随后由调用方补）。
    fn ensure_pool_route(&mut self, agent: &AgentId, mode: PoolMode) -> Result<()> {
        let slug = agent.slug();
        if self
            .config
            .gateway_routes
            .iter()
            .any(|route| route.id == slug)
        {
            return Ok(());
        }
        self.config
            .gateway_routes
            .push(crate::model::gateway::GatewayRoute {
                id: slug.clone(),
                name: t!(Msg::GatewayPoolName, agent = agent.display_name()),
                protocol: provider::downstream_protocol(agent),
                mode: mode.route_mode(),
                members: Vec::new(),
                rules: Vec::new(),
                retry: crate::model::gateway::RetryPolicy::default(),
                enabled: mode.uses_gateway(),
            });
        Ok(())
    }

    fn model_of(&self, provider_id: &str) -> String {
        self.config
            .providers
            .iter()
            .find(|item| item.id == provider_id)
            .and_then(|item| item.models.first().cloned())
            .unwrap_or_default()
    }

    /// 写一条绑定（保留已有槽位映射，避免设默认时把映射清掉）。
    fn write_binding(&mut self, agent: &AgentId, provider_id: &str, model: &str, mode: PoolMode) {
        let slug = agent.slug();
        let slots = self
            .config
            .provider_bindings
            .get(&slug)
            .map(|binding| binding.model_slots.clone())
            .unwrap_or_default();
        self.config.provider_bindings.insert(
            slug.clone(),
            ProviderBinding {
                agent: agent.clone(),
                provider_id: provider_id.to_string(),
                model: model.to_string(),
                make_default: true,
                via_gateway: mode.uses_gateway(),
                route_id: if mode.uses_gateway() {
                    Some(slug)
                } else {
                    None
                },
                mode,
                model_slots: slots,
            },
        );
    }

    /// 网关模式下把 Claude 的别名映射同步进池成员，保证「official name → 上游模型」成对存在。
    ///
    /// 只改默认供应商那个成员：其它成员可能属于别的 agent 语义，不该被覆盖。
    fn sync_alias_map(&mut self, agent: &AgentId, mode: PoolMode) -> Result<()> {
        if !matches!(agent, AgentId::Claude) {
            return Ok(());
        }
        let slug = agent.slug();
        let Some(binding) = self.config.provider_bindings.get(&slug).cloned() else {
            return Ok(());
        };
        // 切回直连时 map 为空 = 只做清理。以前这里直接 `return`，于是直连之后
        // `model_map` 里还留着「官方名 → 上游名」的旧映射：界面上看不出来，
        // 但下次进网关会先带着它跑一轮（用户还以为映射是刚配的）。
        let map = if mode.uses_gateway() {
            crate::ext::gateway::alias_map_for(&binding.model_slots, &binding.model)
        } else {
            BTreeMap::new()
        };
        let Some(route) = self.pool_route_mut(agent) else {
            return Ok(());
        };
        if let Some(member) = route
            .members
            .iter_mut()
            .find(|member| member.provider_id == binding.provider_id)
        {
            // 整组覆盖：用户删掉某个槽位后，旧映射不该残留。
            // 注意元组顺序是 `(槽位名, 官方模型名)`——以前这里写成 `(official, _)`，
            // 取到的是 `sonnet` 这种槽位名，于是「清空」删的是一批根本不存在的键，
            // 旧映射一直留着（`extend` 又只覆盖同名的，用户删掉的槽位永远清不掉）。
            for (_, official) in provider::OFFICIAL_ALIAS {
                member.model_map.remove(official);
                member.model_map.remove(&format!("{official}[1M]"));
                member.model_map.remove(&format!("{official}[1m]"));
            }
            member.model_map.extend(map);
        }
        Ok(())
    }

    /// 直连打供应商的 `/models`；经网关打网关自己的 `/models`（顺带验证网关活着、路由存在、令牌对）。
    fn probe_binding(&self, binding: &ProviderBinding) -> Option<ProbeReport> {
        let provider = self
            .config
            .providers
            .iter()
            .find(|item| item.id == binding.provider_id)?;

        let input = if binding.via_gateway {
            let route_id = binding.route_id.as_deref()?;
            let route = self
                .config
                .gateway_routes
                .iter()
                .find(|item| item.id == route_id)?;
            ProbeInput {
                // 统一入口就是 /v1，`probe_models` 会再拼 `/models`
                base_url: crate::ext::gateway::unified_base_url(self.config.gateway.port),
                api_key: binding.agent.slug(),
                protocol: Some(route.protocol),
            }
        } else {
            ProbeInput {
                base_url: provider.base_url.clone(),
                api_key: provider.api_key.to_reference(),
                protocol: provider.protocol,
            }
        };

        let result = probe_models(&input);
        Some(match result.error {
            Some(error) => ProbeReport {
                endpoint: result.endpoint,
                ok: false,
                detail: error,
                models: 0,
            },
            None => ProbeReport {
                detail: t!(Msg::ProbeOk, count = result.models.len()),
                models: result.models.len(),
                endpoint: result.endpoint,
                ok: true,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market::http::FakeHttp;

    fn probe_input(base_url: &str, protocol: GatewayProtocol) -> ProbeInput {
        ProbeInput {
            base_url: base_url.into(),
            // 明文密钥：测试不依赖任何环境变量
            api_key: "sk-test".into(),
            protocol: Some(protocol),
        }
    }

    #[test]
    fn probe_models_reads_openai_shape() {
        let http = FakeHttp::new(vec![(
            "/v1/models",
            FakeHttp::json(
                r#"{"object":"list","data":[
                    {"id":"deepseek-chat"},
                    {"id":"deepseek-reasoner","context_length":131072,"max_output_tokens":8192}
                ]}"#,
            ),
        )]);
        let result = probe_models_with(
            &http,
            &probe_input(
                "https://api.deepseek.com/v1",
                GatewayProtocol::ChatCompletions,
            ),
        );
        assert!(result.error.is_none(), "{:?}", result.error);
        assert_eq!(result.endpoint, "https://api.deepseek.com/v1/models");
        assert_eq!(result.models.len(), 2);
        let reasoner = result
            .models
            .iter()
            .find(|model| model.id == "deepseek-reasoner")
            .unwrap();
        assert_eq!(reasoner.context, Some(131_072));
        assert_eq!(reasoner.max_output, Some(8_192));
        assert_eq!(
            result.models[0].id, "deepseek-chat",
            "结果按 id 排序，界面顺序稳定"
        );
    }

    #[test]
    fn probe_models_accepts_anthropic_shape_and_string_numbers() {
        let http = FakeHttp::new(vec![(
            "/v1/models",
            FakeHttp::json(
                r#"{"data":[{"type":"model","id":"claude-sonnet-4-5","display_name":"Sonnet","context_window":"200000"}]}"#,
            ),
        )]);
        let result = probe_models_with(
            &http,
            &probe_input("https://api.anthropic.com/v1", GatewayProtocol::Anthropic),
        );
        assert!(result.error.is_none());
        assert_eq!(result.models[0].id, "claude-sonnet-4-5");
        assert_eq!(result.models[0].context, Some(200_000));
        assert_eq!(result.models[0].max_output, None, "上游没给就不要编");
    }

    #[test]
    fn probe_models_reports_readable_errors() {
        let unauthorized = FakeHttp::new(vec![("/models", FakeHttp::status(401))]);
        let result = probe_models_with(
            &unauthorized,
            &probe_input(
                "https://relay.example.com/v1",
                GatewayProtocol::ChatCompletions,
            ),
        );
        assert!(result.error.unwrap().contains("密钥无效"));

        let not_found = FakeHttp::new(vec![("/models", FakeHttp::status(404))]);
        let result = probe_models_with(
            &not_found,
            &probe_input(
                "https://relay.example.com",
                GatewayProtocol::ChatCompletions,
            ),
        );
        assert!(result.error.unwrap().contains("/v1"));

        let html = FakeHttp::new(vec![("/models", FakeHttp::json("<html>hi</html>"))]);
        let result = probe_models_with(
            &html,
            &probe_input(
                "https://relay.example.com/v1",
                GatewayProtocol::ChatCompletions,
            ),
        );
        assert!(result.error.unwrap().contains("不是 JSON"));
    }

    #[test]
    fn probe_endpoint_does_not_duplicate_models() {
        assert_eq!(
            models_endpoint("https://relay.example.com/v1/"),
            "https://relay.example.com/v1/models"
        );
        assert_eq!(
            models_endpoint("https://relay.example.com/v1/models"),
            "https://relay.example.com/v1/models"
        );
    }

    fn input() -> ProviderInput {
        ProviderInput {
            id: "Relay".into(),
            name: " 中转站 ".into(),
            base_url: " https://relay.example.com/v1 ".into(),
            api_key: "env:RELAY_KEY".into(),
            models: vec![" gpt-6.1 ".into(), "".into(), "gpt-6.1".into()],
            note: Some("  ".into()),
            acknowledged: false,
            protocol: None,
            model_limits: BTreeMap::new(),
            catalog_id: None,
        }
    }

    #[test]
    fn build_provider_normalizes_input() {
        let p = build_provider(&input()).unwrap();
        assert_eq!(p.id, "relay", "id 统一小写");
        assert_eq!(p.name, "中转站");
        assert_eq!(p.base_url, "https://relay.example.com/v1");
        assert_eq!(p.models, vec!["gpt-6.1"], "去空去重");
        assert!(p.note.is_none(), "全空白的备注视为没有");
    }

    #[test]
    fn build_provider_rejects_bad_input() {
        let mut bad = input();
        bad.id = "a.b".into();
        assert!(build_provider(&bad).is_err(), "点号会变成 TOML 嵌套表");

        let mut bad = input();
        bad.base_url = "ftp://x".into();
        assert!(build_provider(&bad).is_err());

        let mut bad = input();
        bad.api_key = "sk-plain".into();
        assert!(build_provider(&bad).is_err(), "明文未确认必须拒绝");

        let mut ok = input();
        ok.api_key = "sk-plain".into();
        ok.acknowledged = true;
        assert!(matches!(
            build_provider(&ok).unwrap().api_key,
            SecretRef::Literal {
                acknowledged: true,
                ..
            }
        ));
    }
}
