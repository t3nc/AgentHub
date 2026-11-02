//! 本地 Provider 网关的配置模型 + 功能开关。
//!
//! 这一层只有**数据形状**：序列化格式、默认值、字段含义。路由决策、候选排序、
//! URL 拼接等纯函数在 `ext::gateway`；编排（读写 config.json、拼 ChangeSet）在
//! `api::gateway`；真正跑 HTTP 服务的是独立 crate `agenthub-gateway`。
//!
//! 三条硬约束：
//! - 所有新字段 `#[serde(default)]`：老 `config.json` 加载后等于「全开 + 直连 + 无路由」，
//!   不需要迁移脚本；
//! - 网关只监听回环地址，`host` 不开放给用户改成 `0.0.0.0`；
//! - 这里不存明文密钥 —— 成员只引用 `Provider::id`，密钥解析在转发瞬间发生。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::config::default_true;

/// 网关默认端口。挑一个不常见的号段，避免与 dev server（5173/5273）冲突。
pub const DEFAULT_GATEWAY_PORT: u16 = 43117;

// ---------------------------------------------------------------------------
// 功能开关
// ---------------------------------------------------------------------------

/// 可被「设置 › 功能管理」开关控制的模块。
///
/// 只有这两项：其余模块（技能库 / MCP / 指令 / 项目 / 变更 / Agent 环境 / 终端）
/// 是 AgentHub 的主链路，不做开关。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureKey {
    Provider,
    Usage,
}

impl FeatureKey {
    pub const ALL: [FeatureKey; 2] = [FeatureKey::Provider, FeatureKey::Usage];

    pub fn slug(self) -> &'static str {
        match self {
            FeatureKey::Provider => "provider",
            FeatureKey::Usage => "usage",
        }
    }

    pub fn display_name(self) -> &'static str {
        crate::l10n::str_of(match self {
            FeatureKey::Provider => crate::l10n::Msg::FeatureProvider,
            FeatureKey::Usage => crate::l10n::Msg::FeatureUsage,
        })
    }

    /// 一句话说明，直接用在设置页与禁用页上。
    pub fn description(self) -> &'static str {
        crate::l10n::str_of(match self {
            FeatureKey::Provider => crate::l10n::Msg::FeatureProviderDesc,
            FeatureKey::Usage => crate::l10n::Msg::FeatureUsageDesc,
        })
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "provider" => Some(FeatureKey::Provider),
            "usage" | "tokens" | "token" => Some(FeatureKey::Usage),
            _ => None,
        }
    }
}

/// 功能开关状态。
///
/// 用显式结构体而不是 `BTreeMap<String, bool>`：前端拿到的字段名就是开关的
/// 唯一真源，拼错会直接编译/反序列化失败，而不是静默当成「未启用」。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeatureFlags {
    #[serde(default = "default_true")]
    pub provider: bool,
    #[serde(default = "default_true")]
    pub usage: bool,
}

impl Default for FeatureFlags {
    fn default() -> Self {
        Self {
            provider: true,
            usage: true,
        }
    }
}

impl FeatureFlags {
    pub fn enabled(&self, key: FeatureKey) -> bool {
        match key {
            FeatureKey::Provider => self.provider,
            FeatureKey::Usage => self.usage,
        }
    }

    pub fn set(&mut self, key: FeatureKey, on: bool) {
        match key {
            FeatureKey::Provider => self.provider = on,
            FeatureKey::Usage => self.usage = on,
        }
    }
}

// ---------------------------------------------------------------------------
// 网关配置
// ---------------------------------------------------------------------------

/// 网关进程级配置（与具体路由无关）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    /// 应用启动时自动拉起服务。默认关：用户不点就不占端口。
    #[serde(default)]
    pub auto_start: bool,
    /// 只允许回环地址；写死默认值，界面上不提供改它的入口。
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    /// 本地令牌。`None` = 不校验（只监听回环，风险可接受）。
    ///
    /// 开启后 agent 配置里写的是这个令牌而不是上游密钥，真正做到「上游密钥
    /// 不出现在任何 agent 配置里」。
    #[serde(default)]
    pub auth_token: Option<String>,
    /// 请求元数据是否落 SQLite（`gateway_requests`）。默认关，只在内存里留环形缓冲。
    #[serde(default)]
    pub log_requests: bool,
    /// 环形缓冲上限。
    #[serde(default = "default_max_logs")]
    pub max_log_entries: usize,
}

fn default_host() -> String {
    "127.0.0.1".into()
}

fn default_port() -> u16 {
    DEFAULT_GATEWAY_PORT
}

fn default_max_logs() -> usize {
    500
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            auto_start: false,
            host: default_host(),
            port: default_port(),
            auth_token: None,
            log_requests: false,
            max_log_entries: default_max_logs(),
        }
    }
}

impl GatewayConfig {
    /// 只接受回环地址：任何 `0.0.0.0` / 局域网 IP 都退回默认值。
    ///
    /// 这不是可配置项 —— 把本机全部上游密钥暴露给局域网是明确的安全边界。
    pub fn listen_host(&self) -> &str {
        match self.host.trim() {
            "127.0.0.1" | "localhost" | "::1" => self.host.trim(),
            _ => "127.0.0.1",
        }
    }

    /// 是否启用本地令牌校验。
    pub fn auth_enabled(&self) -> bool {
        self.auth_token
            .as_ref()
            .map(|t| !t.trim().is_empty())
            .unwrap_or(false)
    }
}

// ---------------------------------------------------------------------------
// 路由
// ---------------------------------------------------------------------------

/// 通信协议。
///
/// 既是路由「对外暴露」的协议，也是供应商「上游」的协议 —— 两者不同时，
/// 网关按 `protocol_compatible()` 决定透传、转换还是拒绝。
///
/// 序列化名是显式写死的：`ChatCompletions` 落成 `chat_completions`，
/// 同时用 `alias = "open_ai"` 接住上一版写进 config.json 的旧值，
/// 老用户的 gateway_routes 不需要迁移。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GatewayProtocol {
    /// Anthropic Messages API（Claude Code 的 `/v1/messages`）
    #[serde(rename = "anthropic")]
    Anthropic,
    /// OpenAI Chat Completions（`/v1/chat/completions`）
    #[serde(rename = "chat_completions", alias = "open_ai")]
    ChatCompletions,
    /// OpenAI Responses（`/v1/responses`）。本期只做同协议直通，不参与转换。
    #[serde(rename = "responses")]
    Responses,
}

impl GatewayProtocol {
    pub fn label(self) -> &'static str {
        match self {
            GatewayProtocol::Anthropic => "Anthropic",
            GatewayProtocol::ChatCompletions => "Chat Completions",
            GatewayProtocol::Responses => "Responses",
        }
    }

    /// 该协议在网关里对应的主要路径前缀。
    pub fn default_path(self) -> &'static str {
        match self {
            GatewayProtocol::Anthropic => "/v1/messages",
            GatewayProtocol::ChatCompletions => "/v1/chat/completions",
            GatewayProtocol::Responses => "/v1/responses",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "anthropic" | "messages" => Some(GatewayProtocol::Anthropic),
            "chat_completions" | "open_ai" | "openai" | "chat" => {
                Some(GatewayProtocol::ChatCompletions)
            }
            "responses" => Some(GatewayProtocol::Responses),
            _ => None,
        }
    }
}

/// 一条对外路由的工作方式。
///
/// 「直连」不是这里的一种模式 —— 直连是绑定的选择，请求根本不进网关。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteMode {
    /// 单上游透传：所有请求交给唯一成员。
    Single,
    /// 按模型规则分派；未命中规则时按成员优先级兜底。
    Route,
    /// 多上游池：同模型按权重轮询，失败切下一个。
    Aggregate,
}

/// 供应商池的接入方式 —— 界面上的三模式。
///
/// 与 [`RouteMode`] 的区别：`RouteMode` 描述「经网关时怎么挑成员」，
/// `PoolMode` 描述「要不要经网关」。两者分开是因为「直连」根本不是网关行为。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PoolMode {
    /// 直连：agent 配置里写上游地址与上游密钥，网关不参与
    #[default]
    Direct,
    /// 路由：经本地网关，按模型规则分派到池内成员
    Route,
    /// 聚合：经本地网关，同模型在成员间加权轮询，首字节前失败切下一个
    Aggregate,
}

impl PoolMode {
    pub fn label(self) -> &'static str {
        crate::l10n::str_of(match self {
            PoolMode::Direct => crate::l10n::Msg::PoolDirect,
            PoolMode::Route => crate::l10n::Msg::PoolRoute,
            PoolMode::Aggregate => crate::l10n::Msg::PoolAggregate,
        })
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "direct" | "single" => Some(PoolMode::Direct),
            "route" => Some(PoolMode::Route),
            "aggregate" | "pool" => Some(PoolMode::Aggregate),
            _ => None,
        }
    }

    /// 是否要经本地网关。`Direct` 之外都走网关。
    pub fn uses_gateway(self) -> bool {
        !matches!(self, PoolMode::Direct)
    }

    /// 落到网关侧的路由模式（只有前两个成员时，`Route` 与 `Single` 行为等价）。
    pub fn route_mode(self) -> RouteMode {
        match self {
            PoolMode::Aggregate => RouteMode::Aggregate,
            _ => RouteMode::Route,
        }
    }

    /// 从路由模式反推（老配置迁移 / 界面反查用）。
    pub fn from_route_mode(mode: RouteMode) -> Self {
        match mode {
            RouteMode::Aggregate => PoolMode::Aggregate,
            _ => PoolMode::Route,
        }
    }
}

/// 六个内置 agent 的 slug。
///
/// 这些 id 被用作「供应商池」的路由标识（`/v1` 入口靠令牌 = slug 找池），
/// 所以用户自定义路由不能占用它们。
pub const RESERVED_ROUTE_IDS: [&str; 6] = [
    "claude",
    "codex",
    "codebuddy",
    "hermes",
    "gemini",
    "opencode",
];

impl RouteMode {
    pub fn label(self) -> &'static str {
        crate::l10n::str_of(match self {
            RouteMode::Single => crate::l10n::Msg::RouteSingle,
            RouteMode::Route => crate::l10n::Msg::PoolRoute,
            RouteMode::Aggregate => crate::l10n::Msg::PoolAggregate,
        })
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "single" => Some(RouteMode::Single),
            "route" => Some(RouteMode::Route),
            "aggregate" | "pool" => Some(RouteMode::Aggregate),
            _ => None,
        }
    }
}

/// 路由成员：一条上游 provider 在池子里的角色。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteMember {
    /// 指向 `AppConfig::providers[].id`
    pub provider_id: String,
    /// 聚合模式下的轮询权重；0 视为 1
    #[serde(default = "default_weight")]
    pub weight: u32,
    /// 故障转移顺序，越小越先试
    #[serde(default = "default_priority")]
    pub priority: u32,
    /// 该成员接受的模型白名单；空 = 全部接受
    #[serde(default)]
    pub models: Vec<String>,
    /// 对外模型名 → 上游模型名。为空时请求体字节级不变。
    #[serde(default)]
    pub model_map: BTreeMap<String, String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_weight() -> u32 {
    1
}

fn default_priority() -> u32 {
    100
}

impl RouteMember {
    pub fn weight_or_one(&self) -> u32 {
        self.weight.max(1)
    }

    /// 该成员是否接受这个模型（白名单为空表示全接受）。
    pub fn accepts(&self, model: &str) -> bool {
        if !self.enabled {
            return false;
        }
        if self.models.is_empty() {
            return true;
        }
        self.models
            .iter()
            .any(|pattern| crate::ext::gateway::model_matches(pattern, model))
    }
}

/// 模型规则：命中 `pattern` 的模型只在这些成员里选。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteRule {
    /// 精确模型名，或 `claude-*` 这种尾部通配
    pub pattern: String,
    /// 允许的 `provider_id`；空 = 规则不生效
    #[serde(default)]
    pub members: Vec<String>,
}

/// 重试与超时策略。全部可在路由级覆盖全局默认。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// 总尝试次数（含第一次）；1 = 不重试
    #[serde(default = "default_attempts")]
    pub max_attempts: u32,
    #[serde(default = "default_connect_ms")]
    pub connect_ms: u64,
    /// 首字节超时：LLM 冷启动可能很慢，默认给足 120s
    #[serde(default = "default_first_byte_ms")]
    pub first_byte_ms: u64,
    /// 流式过程中的空闲超时
    #[serde(default = "default_idle_ms")]
    pub idle_ms: u64,
    /// 触发故障转移的状态码
    #[serde(default = "default_retry_status")]
    pub retry_status: Vec<u16>,
}

fn default_attempts() -> u32 {
    3
}
fn default_connect_ms() -> u64 {
    10_000
}
fn default_first_byte_ms() -> u64 {
    120_000
}
fn default_idle_ms() -> u64 {
    180_000
}
fn default_retry_status() -> Vec<u16> {
    vec![408, 409, 425, 429, 500, 502, 503, 504]
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: default_attempts(),
            connect_ms: default_connect_ms(),
            first_byte_ms: default_first_byte_ms(),
            idle_ms: default_idle_ms(),
            retry_status: default_retry_status(),
        }
    }
}

impl RetryPolicy {
    /// 归一化：次数夹到 1..=5，超时给下限，避免手改配置写出 0 次或 0ms。
    pub fn normalized(&self) -> Self {
        Self {
            max_attempts: self.max_attempts.clamp(1, 5),
            connect_ms: self.connect_ms.clamp(1_000, 120_000),
            first_byte_ms: self.first_byte_ms.clamp(1_000, 900_000),
            idle_ms: self.idle_ms.clamp(1_000, 900_000),
            retry_status: if self.retry_status.is_empty() {
                default_retry_status()
            } else {
                self.retry_status.clone()
            },
        }
    }

    pub fn retries(&self) -> bool {
        self.max_attempts > 1
    }

    pub fn should_retry_status(&self, status: u16) -> bool {
        self.retry_status.contains(&status)
    }
}

/// 一条对外路由：`http://127.0.0.1:<port>/gw/<id>/...`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayRoute {
    /// 出现在 URL 路径里；只允许 `[a-z0-9-_]`
    pub id: String,
    pub name: String,
    pub protocol: GatewayProtocol,
    pub mode: RouteMode,
    #[serde(default)]
    pub members: Vec<RouteMember>,
    #[serde(default)]
    pub rules: Vec<RouteRule>,
    #[serde(default)]
    pub retry: RetryPolicy,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl GatewayRoute {
    /// 成员的 provider_id 集合（删除 provider 时用来查引用）。
    pub fn references(&self, provider_id: &str) -> bool {
        self.members.iter().any(|m| m.provider_id == provider_id)
    }
}

/// 设置页「功能管理」一行的数据。
#[derive(Debug, Clone, Serialize)]
pub struct FeatureMeta {
    pub key: &'static str,
    pub display_name: &'static str,
    pub description: &'static str,
    pub enabled: bool,
}

/// 两个开关的完整目录（顺序固定：先 Provider 后 Token 统计）。
///
/// 文案放在后端而不是前端硬编码：禁用页、设置页、CLI 提示共用同一份说法，
/// 免得改了一处忘另一处。
pub fn feature_catalog(flags: &FeatureFlags) -> Vec<FeatureMeta> {
    FeatureKey::ALL
        .iter()
        .map(|key| FeatureMeta {
            key: key.slug(),
            display_name: key.display_name(),
            description: key.description(),
            enabled: flags.enabled(*key),
        })
        .collect()
}
