//! Phase 3：中转站 / 官方 API provider 统一管理（类 cc-switch）。
//!
//! 分工：本模块只做**纯函数**（决定写哪个文件、合并成什么内容），
//! 编排（读配置、解析密钥、拼 `ChangeSet`）在 `api/provider.rs`。
//!
//! 两条硬约束：
//! - **合并而不是覆盖**：`~/.claude/settings.json` 与 `~/.codex/config.toml` 里有用户
//!   大量其它内容（hooks、notify、其它 provider），只动本轮相关的键，其余字节原样保留；
//! - **能不落明文就不落**：Codex 原生支持 `env_key`（写变量名即可），Claude Code 只能写真值，
//!   所以前者全程不解析密钥，后者在 plan 阶段解析、失败即整体中止。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use toml_edit::{DocumentMut, Item, Table, value};

use crate::error::{Error, Result};
use crate::l10n::{Msg, t};
use crate::model::agent::AgentId;
use crate::model::gateway::{GatewayProtocol, PoolMode};
use crate::model::mcp::SecretRef;
use crate::text;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub base_url: String,
    /// 只存引用，不存明文
    pub api_key: SecretRef,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub note: Option<String>,
    /// 上游使用的协议。
    ///
    /// `None` = 未声明：沿用旧行为，网关按**路由的下游协议**直接透传（老配置零迁移）。
    /// 显式声明后才参与跨协议转换，见 `ext::gateway::protocol_compatible`。
    #[serde(default)]
    pub protocol: Option<GatewayProtocol>,
    /// 模型的上下文 / 最大输出长度。只用于展示，不做请求拦截。
    ///
    /// 用独立的 map 而不是把 `models` 改成结构体：后者会让所有老配置反序列化失败。
    #[serde(default)]
    pub model_limits: BTreeMap<String, ModelLimits>,
    /// 渠道来源：内置目录的 id（如 `deepseek`），自定义为空。
    #[serde(default)]
    pub catalog_id: Option<String>,
}

/// 一个模型的长度信息。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelLimits {
    /// 上下文窗口（token）。`None` = 未填
    #[serde(default)]
    pub context: Option<u64>,
    /// 最大输出（token）。`None` = 未填
    #[serde(default)]
    pub max_output: Option<u64>,
    #[serde(default)]
    pub source: ModelLimitSource,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelLimitSource {
    /// 用户手填
    #[default]
    Manual,
    /// 从上游 `/models` 拿到的
    Auto,
}

impl ModelLimits {
    /// 「200K 上下文 · 64K 输出」；两项都没有时返回 None，界面显示「未填」。
    pub fn display(&self) -> Option<String> {
        match (self.context, self.max_output) {
            (None, None) => None,
            (context, output) => {
                let mut parts = Vec::new();
                if let Some(value) = context {
                    parts.push(t!(Msg::ProviderContextSize, tokens = format_tokens(value)));
                }
                if let Some(value) = output {
                    parts.push(t!(Msg::ProviderMaxOutput, tokens = format_tokens(value)));
                }
                Some(parts.join(" · "))
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.context.is_none() && self.max_output.is_none()
    }
}

/// token 数的人类可读形式：`200_000` → `200K`，`1_000_000` → `1M`。
///
/// 不做四舍五入到「万」：128K、200K 这些是官方写法，改了就不好对照。
pub fn format_tokens(tokens: u64) -> String {
    if tokens >= 1_000_000 {
        let value = tokens as f64 / 1_000_000.0;
        if (value - value.round()).abs() < 0.05 {
            format!("{}M", value.round() as u64)
        } else {
            format!("{value:.1}M")
        }
    } else if tokens >= 1_000 {
        let value = tokens as f64 / 1_000.0;
        if (value - value.round()).abs() < 0.05 {
            format!("{}K", value.round() as u64)
        } else {
            format!("{value:.1}K")
        }
    } else {
        tokens.to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderBinding {
    pub agent: AgentId,
    pub provider_id: String,
    pub model: String,
    /// 是否把该 binding 设为该 agent 的当前默认
    #[serde(default)]
    pub make_default: bool,
    /// 是否经本地网关。
    ///
    /// `false` = 直连：配置里写上游 base_url 与上游密钥（原行为，零额外开销）；
    /// `true` = 写 `http://127.0.0.1:<port>/gw/<route>` 与网关令牌，上游密钥不出现在
    /// agent 配置里。
    #[serde(default)]
    pub via_gateway: bool,
    /// 经网关时走哪条路由；`None` 且 `via_gateway = true` 时生成计划会明确报错。
    ///
    /// 新布局下它等于 agent 的 slug（每个 agent 一条「供应商池」路由）。
    #[serde(default)]
    pub route_id: Option<String>,
    /// 三模式：直连 / 路由 / 聚合。
    ///
    /// 默认 `Direct`，老配置没有这个字段时按直连处理。
    #[serde(default)]
    pub mode: PoolMode,
    /// Claude 的模型槽位映射：`sonnet` / `opus` / `haiku` / `fable` → 上游模型名。
    ///
    /// 空 map 时四个别名都等于 `model`（与改造前一致）。其它 agent 忽略它。
    #[serde(default)]
    pub model_slots: BTreeMap<String, String>,
}

impl ProviderBinding {
    /// 归一化后的模式。
    ///
    /// 老配置只有 `via_gateway`：`true` 表示当时走了网关，但那时还没有三模式概念，
    /// 一律按「路由」处理，避免升级后突然变成直连而断掉正在跑的中转。
    pub fn effective_mode(&self) -> PoolMode {
        if self.mode == PoolMode::Direct && self.via_gateway {
            PoolMode::Route
        } else {
            self.mode
        }
    }

    /// 该 agent 的供应商池使用的路由 id：固定等于 slug。
    pub fn pool_route_id(&self) -> String {
        self.agent.slug()
    }

    /// 取某个槽位的上游模型名，未配置时回落默认模型。
    pub fn slot_model(&self, slot: &str) -> &str {
        self.model_slots
            .get(slot)
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .unwrap_or(&self.model)
    }
}

/// provider 配置的写盘格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetFormat {
    Json,
    Toml,
}

/// 合并策略。不同 agent 的键位置差异太大，用枚举显式区分，避免在编排层堆 `match agent`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetKind {
    /// Claude Code：`settings.json` 的 `env.*`，注释与无关键全部保留
    ClaudeSettings,
    /// Codex：`config.toml` 的 `[model_providers.<id>]`，`toml_edit` 保注释
    CodexToml,
    /// OpenCode：`provider.<id>` 整棵子树（`npm` / `name` / `options` / `models`）。
    ///
    /// 不能用通用点路径硬凑：`models` 是「动态键名 + 嵌套对象」，
    /// 而且**缺了 `npm` 和 `models`，OpenCode 根本不会加载这个 provider**
    /// （本机可用的 `agnes-ai` 两者都有，AgentHub 写的 `agnescn` 两者都没有）。
    OpencodeJson,
    /// 通用 JSON 点路径（保留给以后接入的格式）
    JsonKeys,
}

/// 一个 agent 的 provider 配置落点。
///
/// 键路径里的 `{id}` 会被替换成 provider id（Codex / OpenCode 需要把 provider
/// 作为一级键）。
#[derive(Debug, Clone)]
pub struct ProviderTarget {
    pub file: PathBuf,
    pub format: TargetFormat,
    pub kind: TargetKind,
    pub base_url_key: String,
    pub api_key_key: String,
    /// `None` = 该 agent 的模型选择不写在这个文件里（或键名尚未核实）。
    /// 这时只写地址与密钥，绑定里的模型只用于生成提示。
    pub model_key: Option<String>,
    /// `true` = 密钥写真值；`false` = 只写变量名（Codex 的 `env_key`、Hermes 的 `key_env`）
    pub key_is_literal: bool,
}

/// 本轮真正接入了 provider 写入的 agent。
///
/// 判断依据是**实证**：本机真实配置文件里读到的键名，或者官方 schema 明确写死的结构。
/// 核实不了的一律拒绝，绝不写一份「看起来对」的配置。
pub fn supports(agent: &AgentId) -> bool {
    matches!(agent, AgentId::Claude | AgentId::Codex | AgentId::Opencode)
}

/// agent 对下游说哪种协议 —— 也就是它的「供应商池」路由该用的 `protocol`。
///
/// 网关的协议转换是按「下游协议 × 上游协议」决定的，所以池路由必须跟 agent 自己
/// 说的协议一致，否则转换方向会反。
pub fn downstream_protocol(agent: &AgentId) -> GatewayProtocol {
    match agent {
        AgentId::Codex => GatewayProtocol::Responses,
        AgentId::Opencode => GatewayProtocol::ChatCompletions,
        _ => GatewayProtocol::Anthropic,
    }
}

/// 这个 agent 能不能吃下某种上游协议。
///
/// `via_gateway` 决定「Claude + Chat 上游」是否可行：网关会做 Anthropic ⇄ Chat
/// 转换；直连时 Claude 只会说 Anthropic，硬写过去一定失败。
///
/// 边界来自各 CLI 真实支持的 wire 协议（Codex 的 `wire_api` 只有 chat / responses），
/// 不是猜测。返回的错误文案可以直接展示给用户。
pub fn agent_accepts_protocol(
    agent: &AgentId,
    protocol: Option<GatewayProtocol>,
    via_gateway: bool,
) -> Result<()> {
    // 未声明协议：三家都能按各自的历史默认直连，不拦
    let Some(protocol) = protocol else {
        return Ok(());
    };
    match agent {
        AgentId::Claude => match protocol {
            GatewayProtocol::Anthropic => Ok(()),
            GatewayProtocol::ChatCompletions if via_gateway => Ok(()),
            GatewayProtocol::ChatCompletions => {
                Err(Error::Unsupported(t!(Msg::ProviderClaudeChatDirect)))
            }
            GatewayProtocol::Responses => Err(Error::Unsupported(t!(Msg::ProviderClaudeResponses))),
        },
        AgentId::Codex => match protocol {
            GatewayProtocol::Anthropic => Err(Error::Unsupported(t!(Msg::ProviderCodexAnthropic))),
            GatewayProtocol::ChatCompletions | GatewayProtocol::Responses => Ok(()),
        },
        // OpenCode 靠 npm 适配器声明协议，三种都能直连
        AgentId::Opencode => Ok(()),
        _ => Ok(()),
    }
}

/// 该 agent 的配置是「写密钥真值」还是「只写变量名」。
///
/// Codex 的 `config.toml` 只有 `env_key`；其余已接入的格式（Claude / OpenCode）
/// 都接受真值。绑定时的前置校验用它，避免先落一条永远无法应用的绑定。
pub fn key_is_literal(agent: &AgentId) -> bool {
    !matches!(agent, AgentId::Codex)
}

pub fn unsupported_reason(agent: &AgentId) -> String {
    match agent {
        AgentId::Gemini => t!(Msg::ProviderStatusGemini),
        AgentId::CodeBuddy => t!(Msg::ProviderStatusCodebuddy),
        AgentId::Hermes => t!(Msg::ProviderStatusHermes),
        AgentId::Custom(_) => t!(Msg::ProviderStatusCustom),
        other => t!(Msg::ProviderStatusOther, agent = other.display_name()),
    }
}

/// 写盘目标。
///
/// 用适配器给出的 `config_dir` 定位，而不是自己拼家目录 —— 这样设置页的路径覆盖
/// （用户把 `~/.claude` 指到别处）对 Provider 同样生效。
pub fn target_path(agent: &AgentId, config_dir: Option<&Path>) -> Result<PathBuf> {
    if let Some(spec) = spec_for(agent, config_dir) {
        return Ok(spec.file);
    }
    if !supports(agent) {
        return Err(Error::Unsupported(unsupported_reason(agent)));
    }
    Err(Error::NotFound(t!(
        Msg::ErrProviderNoConfigDir,
        agent = agent.display_name()
    )))
}

/// 按 agent 与配置目录构造落点描述。
///
/// 键路径都来自本机真实文件：
/// - Claude Code `~/.claude/settings.json` 的 `env.ANTHROPIC_BASE_URL` /
///   `env.ANTHROPIC_AUTH_TOKEN` / `env.ANTHROPIC_MODEL`，外加 4 个 alias pin；
/// - Codex `~/.codex/config.toml` 的根级 `model` / `model_provider` 与
///   `[model_providers.<id>]`（`base_url` + `env_key` + `wire_api`）；
/// - OpenCode `~/.config/opencode/opencode.json` 的 `provider.<id>`：
///   `npm` + `name` + `options` + `models`。
///   顶层 `model`（`provider/model`）在本机真实文件里 0 处出现，因此不写。
pub fn spec_for(agent: &AgentId, config_dir: Option<&Path>) -> Option<ProviderTarget> {
    let dir = config_dir?;
    match agent {
        AgentId::Claude => Some(ProviderTarget {
            file: dir.join("settings.json"),
            format: TargetFormat::Json,
            kind: TargetKind::ClaudeSettings,
            base_url_key: "env.ANTHROPIC_BASE_URL".into(),
            api_key_key: "env.ANTHROPIC_AUTH_TOKEN".into(),
            model_key: Some("env.ANTHROPIC_MODEL".into()),
            key_is_literal: true,
        }),
        AgentId::Codex => Some(ProviderTarget {
            file: dir.join("config.toml"),
            format: TargetFormat::Toml,
            kind: TargetKind::CodexToml,
            base_url_key: "model_providers.{id}.base_url".into(),
            api_key_key: "model_providers.{id}.env_key".into(),
            model_key: Some("model".into()),
            key_is_literal: false,
        }),
        AgentId::Opencode => Some(ProviderTarget {
            file: dir.join("opencode.json"),
            format: TargetFormat::Json,
            kind: TargetKind::OpencodeJson,
            base_url_key: "provider.{id}.options.baseURL".into(),
            api_key_key: "provider.{id}.options.apiKey".into(),
            model_key: None,
            key_is_literal: true,
        }),
        _ => None,
    }
}

/// OpenCode 的 SDK 适配器声明。**这是 provider 能不能被加载的开关**。
///
/// 三种协议的映射依据本机 `opencode.json` 里能用的 `agnes-ai`（Chat 兼容层）
/// 以及 `@ai-sdk/*` 的命名约定；未声明协议时按 OpenAI 兼容兜底
/// （OpenCode 接入第三方中转站的主场景）。
pub fn npm_for(protocol: Option<GatewayProtocol>) -> &'static str {
    match protocol {
        Some(GatewayProtocol::Anthropic) => "@ai-sdk/anthropic",
        Some(GatewayProtocol::Responses) => "@ai-sdk/openai",
        Some(GatewayProtocol::ChatCompletions) | None => "@ai-sdk/openai-compatible",
    }
}

/// Codex 的 `wire_api`。本机所有可用配置都显式写了它（全是 `responses`），
/// 不写时 Codex 用哪个属于未定义行为，所以永远写出来。
pub fn wire_api_for(protocol: Option<GatewayProtocol>) -> &'static str {
    match protocol {
        Some(GatewayProtocol::ChatCompletions) => "chat",
        // Codex 的历史默认就是 responses；Anthropic 上游在矩阵层就被拦掉了
        Some(GatewayProtocol::Responses) | Some(GatewayProtocol::Anthropic) | None => "responses",
    }
}

/// 一次写入所需的全部信息（密钥已按 agent 的格式决定是「变量名」还是「真值」）。
pub struct ProviderWrite<'a> {
    pub provider: &'a Provider,
    pub model: &'a str,
    pub make_default: bool,
    /// Claude 的 `settings.json` 只能写真值；Codex 走 `env_key` 时传 `None`
    pub api_key_value: Option<&'a str>,
    /// 真正写进 agent 配置的地址。
    ///
    /// 直连 = 供应商自己登记的 `base_url`；经网关 = `http://127.0.0.1:<port>/v1`。
    /// 由编排层算好传进来，合并函数不关心端口从哪来。
    pub base_url: &'a str,
    /// 三模式：决定 Claude 的四个 alias pin 写上游名还是官方名。
    pub mode: PoolMode,
    /// Claude 的模型槽位映射（`sonnet` / `opus` / … → 上游模型名）；其它 agent 忽略。
    pub slots: &'a BTreeMap<String, String>,
}

/// 经网关时 Claude 四个家族别名要写的官方模型 id。
///
/// 取自本机 `~/.claude/settings.json.bak` 与 `daemon/roster.json` 的真实取值。
/// 网关侧再用 `model_map` 把官方名换成上游名 —— 这样 Claude Code 的
/// 上下文窗口 / effort / thinking 能力检测仍然按官方模型走。
pub const OFFICIAL_ALIAS: [(&str, &str); 4] = [
    ("sonnet", "claude-sonnet-5"),
    ("opus", "claude-opus-5"),
    ("haiku", "claude-haiku-5-5"),
    ("fable", "claude-fable-5"),
];

/// 槽位白名单。顺序即界面顺序。
pub const SLOTS: [&str; 4] = ["sonnet", "opus", "haiku", "fable"];

impl<'a> ProviderWrite<'a> {
    /// 某个槽位最终要写的模型名：直连写上游名，网关模式写官方别名。
    fn slot_value(&self, slot: &str) -> String {
        if self.mode.uses_gateway() {
            OFFICIAL_ALIAS
                .iter()
                .find(|(key, _)| *key == slot)
                .map(|(_, official)| official.to_string())
                .unwrap_or_else(|| self.model.to_string())
        } else {
            self.slots
                .get(slot)
                .map(|value| value.trim())
                .filter(|value| !value.is_empty())
                .unwrap_or(self.model)
                .to_string()
        }
    }
}

/// 合并写入 Claude Code 的 `settings.json`。
///
/// 只写 `env.ANTHROPIC_BASE_URL` / `ANTHROPIC_AUTH_TOKEN` / `ANTHROPIC_MODEL`，
/// 以及 `make_default` 时的顶层 `model`；其余键（hooks、statusLine、permissions…）原样保留。
pub fn merge_claude_settings(existing: Option<&str>, write: &ProviderWrite<'_>) -> Result<String> {
    let had_bom = existing.map(text::has_bom).unwrap_or(false);
    let clean = existing.map(text::strip_bom);
    let mut doc: Value = match clean {
        Some(t) if !t.trim().is_empty() => serde_json::from_str(t)
            .map_err(|e| Error::msg(t!(Msg::ErrSettingsNotJson, error = e)))?,
        _ => json!({}),
    };
    {
        let root = doc
            .as_object_mut()
            .ok_or_else(|| Error::msg(t!(Msg::ErrSettingsNotObject)))?;
        let env = root
            .entry("env")
            .or_insert_with(|| Value::Object(Map::new()));
        let env_map = env
            .as_object_mut()
            .ok_or_else(|| Error::msg(t!(Msg::ErrSettingsEnvNotObject)))?;
        env_map.insert("ANTHROPIC_BASE_URL".into(), json!(write.base_url.trim()));
        // 网关模式下起始会话也写官方别名，跟四个 pin 保持一致
        let start_model = write.slot_value("sonnet");
        env_map.insert("ANTHROPIC_MODEL".into(), json!(start_model));
        // 4 个 alias pin：用户在 `/model` 里选 sonnet / opus / haiku / fable 时，
        // 别名会被重定向到这里的值。**四个槽位各写各的**，切换才真的换模型；
        // 同时清掉 `_MODEL_NAME`（那个键只改选择器里显示的文字，会让人
        // 看到 A 实际发 B）。
        for (slot, _) in OFFICIAL_ALIAS {
            let value = write.slot_value(slot);
            if value.trim().is_empty() {
                continue;
            }
            let upper = slot.to_uppercase();
            env_map.insert(format!("ANTHROPIC_DEFAULT_{upper}_MODEL"), json!(value));
            env_map.remove(&format!("ANTHROPIC_DEFAULT_{upper}_MODEL_NAME"));
        }
        if let Some(key) = write.api_key_value {
            // 两个键都写：本机 live 文件里是 API_KEY，Pebrel 写的是 AUTH_TOKEN，
            // 不同版本/工具读的不是同一个，写全比猜一个稳。
            env_map.insert("ANTHROPIC_AUTH_TOKEN".into(), json!(key));
            env_map.insert("ANTHROPIC_API_KEY".into(), json!(key));
        }
    }
    if write.make_default
        && let Some(root) = doc.as_object_mut()
    {
        root.insert("model".into(), json!(write.model));
    }
    let rendered = serde_json::to_string_pretty(&doc).map_err(Error::Json)?;
    Ok(restore_json_eol(clean, rendered, had_bom))
}

/// 合并写入 Codex 的 `config.toml`。
///
/// 根级只动 `model` / `model_provider`，表级只动 `[model_providers.<id>]`；
/// 其它 provider 子表、`[mcp_servers.*]`、注释全部由 `toml_edit` 原样保留。
pub fn merge_codex_config(existing: Option<&str>, write: &ProviderWrite<'_>) -> Result<String> {
    let had_bom = existing.map(text::has_bom).unwrap_or(false);
    let clean = existing.map(text::strip_bom);
    let mut doc: DocumentMut = match clean {
        Some(t) if !t.trim().is_empty() => t
            .parse()
            .map_err(|e| Error::msg(t!(Msg::ErrTomlParse, error = e)))?,
        _ => DocumentMut::new(),
    };

    let id = write.provider.id.trim();
    if id.is_empty() {
        return Err(Error::msg(t!(Msg::ErrProviderIdEmpty)));
    }

    doc["model"] = value(write.model);
    doc["model_provider"] = value(id);
    if !doc.contains_key("model_providers") {
        let mut table = Table::new();
        table.set_implicit(false);
        doc["model_providers"] = Item::Table(table);
    }
    let providers = doc["model_providers"]
        .as_table_mut()
        .ok_or_else(|| Error::msg(t!(Msg::ErrModelProvidersNotTable)))?;

    let mut entry = Table::new();
    entry.insert("name", value(write.provider.name.trim()));
    entry.insert("base_url", value(write.base_url.trim()));
    // 必须显式声明：Chat 上游要 `chat`，否则 Codex 会按 responses 打过去
    entry.insert("wire_api", value(wire_api_for(write.provider.protocol)));
    // Codex 用变量名引用密钥：这里写入的是 `env:VAR` 里的 VAR，不是密文本身
    if let SecretRef::Env { var } = &write.provider.api_key {
        entry.insert("env_key", value(var.clone()));
    }
    providers.insert(id, Item::Table(entry));

    let eol = clean.map(text::eol_of).unwrap_or("\n");
    let mut out = text::with_eol(&doc.to_string(), eol);
    if !out.ends_with(eol) {
        out.push_str(eol);
    }
    Ok(text::with_bom_if(had_bom, out))
}

/// 合并写入 OpenCode 的 `opencode.json`。
///
/// 写 `provider.<id>` 一整棵子树：
/// - `npm`：SDK 适配器声明，**缺了它 OpenCode 不会加载这个 provider**；
/// - `name`；
/// - `options.baseURL` / `options.apiKey`；
/// - `models`：至少包含当前绑定的模型；已有的其它模型条目原样保留
///   （用户可能手工加过），有长度登记时补 `options.contextWindow` / `maxOutputTokens`。
///
/// 只碰 `provider.<id>`，`$schema`、别的 provider、`mcp` 段一律不动。
pub fn merge_opencode_config(existing: Option<&str>, write: &ProviderWrite<'_>) -> Result<String> {
    let had_bom = existing.map(text::has_bom).unwrap_or(false);
    let clean = existing.map(text::strip_bom);
    let mut doc: Value = match clean {
        Some(t) if !t.trim().is_empty() => serde_json::from_str(t)
            .map_err(|e| Error::msg(t!(Msg::ErrOpencodeNotJson, error = e)))?,
        _ => json!({}),
    };
    if !doc.is_object() {
        return Err(Error::msg(t!(Msg::ErrOpencodeNotObject)));
    }

    let id = write.provider.id.trim();
    if id.is_empty() {
        return Err(Error::msg(t!(Msg::ErrProviderIdEmpty)));
    }

    ensure_object(&mut doc, &["provider", id])?;
    ensure_object(&mut doc, &["provider", id, "options"])?;
    ensure_object(&mut doc, &["provider", id, "models"])?;

    doc["provider"][id]["npm"] = json!(npm_for(write.provider.protocol));
    doc["provider"][id]["name"] = json!(write.provider.name.trim());
    doc["provider"][id]["options"]["baseURL"] = json!(write.base_url.trim());
    if let Some(key) = write.api_key_value {
        doc["provider"][id]["options"]["apiKey"] = json!(key);
    }

    let model = write.model.trim();
    if !model.is_empty() {
        ensure_object(&mut doc, &["provider", id, "models", model])?;
        // 只补 name，不覆盖用户自己起的中文名
        if doc["provider"][id]["models"][model].get("name").is_none() {
            doc["provider"][id]["models"][model]["name"] = json!(model);
        }
        if let Some(limits) = write.provider.model_limits.get(model)
            && (limits.context.is_some() || limits.max_output.is_some())
        {
            ensure_object(&mut doc, &["provider", id, "models", model, "options"])?;
            if let Some(context) = limits.context {
                doc["provider"][id]["models"][model]["options"]["contextWindow"] = json!(context);
            }
            if let Some(output) = limits.max_output {
                doc["provider"][id]["models"][model]["options"]["maxOutputTokens"] = json!(output);
            }
        }
    }

    let rendered = serde_json::to_string_pretty(&doc).map_err(Error::Json)?;
    Ok(restore_json_eol(clean, rendered, had_bom))
}

/// 逐层确保是对象（不存在就建，是标量就覆盖成对象）。
fn ensure_object(doc: &mut Value, parts: &[&str]) -> Result<()> {
    let mut cursor = doc;
    for part in parts {
        if !cursor.is_object() {
            *cursor = json!({});
        }
        cursor = cursor
            .as_object_mut()
            .ok_or_else(|| Error::msg(t!(Msg::ErrConfigNodeNotObject)))?
            .entry((*part).to_string())
            .or_insert_with(|| json!({}));
    }
    if !cursor.is_object() {
        *cursor = json!({});
    }
    Ok(())
}

/// 按 agent 的落点描述选择合并策略。
pub fn merge_for(
    spec: &ProviderTarget,
    existing: Option<&str>,
    write: &ProviderWrite<'_>,
) -> Result<String> {
    match spec.kind {
        TargetKind::ClaudeSettings => merge_claude_settings(existing, write),
        TargetKind::CodexToml => merge_codex_config(existing, write),
        TargetKind::OpencodeJson => merge_opencode_config(existing, write),
        TargetKind::JsonKeys => merge_json_keys(existing, write, spec),
    }
}

/// 通用 JSON 点路径合并（OpenCode 的 `provider.<id>.options.*` 这类结构）。
///
/// 只写 spec 声明过的路径，其余键（`$schema`、别的 provider、主题配置）原样保留。
/// `api_key_value` 为 `None` 时连密钥键都不碰 —— 宁可少写一个键，也不写一个空值。
pub fn merge_json_keys(
    existing: Option<&str>,
    write: &ProviderWrite<'_>,
    spec: &ProviderTarget,
) -> Result<String> {
    let had_bom = existing.map(text::has_bom).unwrap_or(false);
    let clean = existing.map(text::strip_bom);
    let mut doc: Value = match clean {
        Some(t) if !t.trim().is_empty() => {
            serde_json::from_str(t).map_err(|e| Error::msg(t!(Msg::ErrConfigNotJson, error = e)))?
        }
        _ => json!({}),
    };
    if !doc.is_object() {
        return Err(Error::msg(t!(Msg::ErrConfigNotObject)));
    }

    let id = write.provider.id.trim();
    if id.is_empty() {
        return Err(Error::msg(t!(Msg::ErrProviderIdEmpty)));
    }
    let base_key = spec.base_url_key.replace("{id}", id);
    let key_key = spec.api_key_key.replace("{id}", id);

    write_path(&mut doc, &base_key, json!(write.provider.base_url.trim()))?;
    if let Some(value) = write.api_key_value {
        write_path(&mut doc, &key_key, json!(value))?;
    }
    if write.make_default
        && let Some(model_key) = spec.model_key.as_deref()
    {
        write_path(&mut doc, model_key, json!(write.model))?;
    }

    let rendered = serde_json::to_string_pretty(&doc).map_err(Error::Json)?;
    Ok(restore_json_eol(clean, rendered, had_bom))
}

/// 按点路径写入，逐层创建对象。遇到「该位置已经是标量」时直接覆盖成对象。
fn write_path(doc: &mut Value, path: &str, value: Value) -> Result<()> {
    let parts: Vec<&str> = path.split('.').filter(|part| !part.is_empty()).collect();
    if parts.is_empty() {
        return Err(Error::msg(t!(Msg::ErrBadKeyPath, path = path)));
    }
    let mut cursor = doc;
    for (index, part) in parts.iter().enumerate() {
        if !cursor.is_object() {
            *cursor = json!({});
        }
        let map = cursor
            .as_object_mut()
            .ok_or_else(|| Error::msg(t!(Msg::ErrConfigNodeNotObject)))?;
        if index + 1 == parts.len() {
            map.insert((*part).to_string(), value);
            return Ok(());
        }
        cursor = map.entry((*part).to_string()).or_insert_with(|| json!({}));
    }
    Ok(())
}

/// `serde_json` 只输出 LF 且不带结尾换行：按原文件风格还原，避免整文件 diff。
fn restore_json_eol(original: Option<&str>, rendered: String, had_bom: bool) -> String {
    let eol = original.map(text::eol_of).unwrap_or("\n");
    let want_trailing = original.map(|t| t.ends_with('\n')).unwrap_or(true);
    let mut out = text::with_eol(&rendered, eol);
    if want_trailing && !out.ends_with(eol) {
        out.push_str(eol);
    }
    text::with_bom_if(had_bom, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider(secret: SecretRef) -> Provider {
        Provider {
            id: "relay".into(),
            name: "中转站".into(),
            base_url: "https://relay.example.com/v1".into(),
            api_key: secret,
            models: vec!["claude-sonnet-4-5".into()],
            note: None,
            protocol: None,
            model_limits: BTreeMap::new(),
            catalog_id: None,
        }
    }

    /// 直连模式的写入参数：地址取供应商自己的，无槽位映射。
    ///
    /// 测试里十来个用例都是这个形态，用一个构造器避免每处重复三个字段。
    fn plain<'a>(
        provider: &'a Provider,
        model: &'a str,
        make_default: bool,
        api_key_value: Option<&'a str>,
    ) -> ProviderWrite<'a> {
        static EMPTY: std::sync::LazyLock<BTreeMap<String, String>> =
            std::sync::LazyLock::new(BTreeMap::new);
        ProviderWrite {
            provider,
            model,
            make_default,
            api_key_value,
            base_url: provider.base_url.as_str(),
            mode: PoolMode::Direct,
            slots: &EMPTY,
        }
    }

    fn provider_with(protocol: Option<GatewayProtocol>) -> Provider {
        let mut item = provider(SecretRef::Env {
            var: "RELAY_KEY".into(),
        });
        item.protocol = protocol;
        item
    }

    #[test]
    fn npm_and_wire_api_follow_protocol() {
        assert_eq!(
            npm_for(Some(GatewayProtocol::ChatCompletions)),
            "@ai-sdk/openai-compatible"
        );
        assert_eq!(
            npm_for(Some(GatewayProtocol::Anthropic)),
            "@ai-sdk/anthropic"
        );
        assert_eq!(npm_for(Some(GatewayProtocol::Responses)), "@ai-sdk/openai");
        assert_eq!(
            npm_for(None),
            "@ai-sdk/openai-compatible",
            "未声明按 Chat 兼容兜底"
        );

        assert_eq!(wire_api_for(Some(GatewayProtocol::ChatCompletions)), "chat");
        assert_eq!(wire_api_for(Some(GatewayProtocol::Responses)), "responses");
        assert_eq!(wire_api_for(None), "responses", "Codex 的历史默认值");
    }

    #[test]
    fn opencode_keeps_handwritten_models_and_fills_limits() {
        let original = concat!(
            "{\n",
            "  \"provider\": {\n",
            "    \"relay\": {\n",
            "      \"models\": {\n",
            "        \"keep-me\": { \"name\": \"我自己起的名字\" }\n",
            "      }\n",
            "    }\n",
            "  }\n",
            "}\n"
        );
        let mut p = provider_with(Some(GatewayProtocol::ChatCompletions));
        p.model_limits.insert(
            "claude-sonnet-4-5".into(),
            ModelLimits {
                context: Some(200_000),
                max_output: Some(64_000),
                source: ModelLimitSource::Auto,
            },
        );
        let write = plain(&p, "claude-sonnet-4-5", true, Some("sk-test"));
        let spec = spec_for(&AgentId::Opencode, Some(Path::new("/tmp/oc"))).unwrap();
        let doc: Value =
            serde_json::from_str(&merge_for(&spec, Some(original), &write).unwrap()).unwrap();

        assert_eq!(
            doc["provider"]["relay"]["models"]["keep-me"]["name"],
            json!("我自己起的名字"),
            "用户手工加的模型不能被覆盖"
        );
        assert_eq!(
            doc["provider"]["relay"]["models"]["claude-sonnet-4-5"]["options"]["contextWindow"],
            json!(200_000)
        );
        assert_eq!(
            doc["provider"]["relay"]["models"]["claude-sonnet-4-5"]["options"]["maxOutputTokens"],
            json!(64_000)
        );
    }

    #[test]
    fn claude_pins_all_four_aliases() {
        let p = provider(SecretRef::Literal {
            value: "sk-ant".into(),
            acknowledged: true,
        });
        let write = plain(&p, "deepseek-v4-flash", true, Some("sk-ant"));
        let spec = spec_for(&AgentId::Claude, Some(Path::new("/tmp/claude"))).unwrap();
        let doc: Value = serde_json::from_str(&merge_for(&spec, None, &write).unwrap()).unwrap();
        let env = &doc["env"];

        assert_eq!(
            env["ANTHROPIC_BASE_URL"],
            json!("https://relay.example.com/v1")
        );
        assert_eq!(env["ANTHROPIC_MODEL"], json!("deepseek-v4-flash"));
        for alias in ["SONNET", "OPUS", "HAIKU", "FABLE"] {
            assert_eq!(
                env[format!("ANTHROPIC_DEFAULT_{alias}_MODEL")],
                json!("deepseek-v4-flash"),
                "{alias} 别名必须指向同一个模型，否则用户选它时会绕回官方模型"
            );
        }
        // 两个认证键都写：不同版本读的不是同一个
        assert_eq!(env["ANTHROPIC_AUTH_TOKEN"], json!("sk-ant"));
        assert_eq!(env["ANTHROPIC_API_KEY"], json!("sk-ant"));
    }

    #[test]
    fn codex_writes_wire_api_for_chat_upstream() {
        let p = provider_with(Some(GatewayProtocol::ChatCompletions));
        let write = plain(&p, "deepseek-chat", true, None);
        let spec = spec_for(&AgentId::Codex, Some(Path::new("/tmp/codex"))).unwrap();
        let parsed: DocumentMut = merge_for(&spec, None, &write).unwrap().parse().unwrap();

        assert_eq!(parsed["model"].as_str(), Some("deepseek-chat"));
        assert_eq!(parsed["model_provider"].as_str(), Some("relay"));
        assert_eq!(
            parsed["model_providers"]["relay"]["wire_api"].as_str(),
            Some("chat"),
            "Chat 上游必须显式 chat，否则 Codex 按 responses 打过去会失败"
        );
        assert_eq!(
            parsed["model_providers"]["relay"]["env_key"].as_str(),
            Some("RELAY_KEY")
        );
    }

    #[test]
    fn protocol_matrix_matches_cli_capabilities() {
        use GatewayProtocol::{Anthropic, ChatCompletions, Responses};

        // Claude：直连只会说 Anthropic；Chat 上游必须经网关；Responses 不支持
        assert!(agent_accepts_protocol(&AgentId::Claude, Some(Anthropic), false).is_ok());
        assert!(
            agent_accepts_protocol(&AgentId::Claude, Some(ChatCompletions), false).is_err(),
            "Claude 直连 Chat 上游一定失败，必须在绑定前拦住"
        );
        assert!(agent_accepts_protocol(&AgentId::Claude, Some(ChatCompletions), true).is_ok());
        assert!(agent_accepts_protocol(&AgentId::Claude, Some(Responses), true).is_err());

        // Codex：只有 chat / responses，接不了 Anthropic（网关也无济于事）
        assert!(agent_accepts_protocol(&AgentId::Codex, Some(Anthropic), true).is_err());
        assert!(agent_accepts_protocol(&AgentId::Codex, Some(ChatCompletions), false).is_ok());
        assert!(agent_accepts_protocol(&AgentId::Codex, Some(Responses), false).is_ok());

        // OpenCode：靠 npm 适配器表达协议，三种都能直连
        for protocol in [Anthropic, ChatCompletions, Responses] {
            assert!(
                agent_accepts_protocol(&AgentId::Opencode, Some(protocol), false).is_ok(),
                "OpenCode 应支持 {protocol:?}"
            );
        }

        // 未声明协议：按各自历史默认直连，不拦
        assert!(agent_accepts_protocol(&AgentId::Claude, None, false).is_ok());
        assert!(agent_accepts_protocol(&AgentId::Codex, None, false).is_ok());
    }

    #[test]
    fn codex_defaults_to_responses_when_protocol_undeclared() {
        let p = provider_with(None);
        let write = plain(&p, "gpt-6.1", true, None);
        let spec = spec_for(&AgentId::Codex, Some(Path::new("/tmp/codex"))).unwrap();
        let parsed: DocumentMut = merge_for(&spec, None, &write).unwrap().parse().unwrap();
        assert_eq!(
            parsed["model_providers"]["relay"]["wire_api"].as_str(),
            Some("responses")
        );
    }

    #[test]
    fn token_counts_are_humanized() {
        assert_eq!(format_tokens(8_192), "8.2K");
        assert_eq!(format_tokens(128_000), "128K");
        assert_eq!(format_tokens(200_000), "200K");
        assert_eq!(format_tokens(1_000_000), "1M");
        assert_eq!(format_tokens(1_048_576), "1M", "1024² 按十进制也归到 1M");
        assert_eq!(format_tokens(1_500_000), "1.5M");
        assert_eq!(format_tokens(512), "512");
    }

    #[test]
    fn model_limits_display_skips_missing_parts() {
        // 断言结构（数值与「两段之间用 · 连接」），不断言具体语言：文案随界面语言变化
        let both = ModelLimits {
            context: Some(200_000),
            max_output: Some(64_000),
            source: ModelLimitSource::Auto,
        };
        let text = both.display().expect("两项都有时必须能显示");
        assert!(
            text.contains("200K") && text.contains("64K"),
            "实际：{text}"
        );
        assert!(text.contains(" · "), "两段之间应有分隔符，实际：{text}");

        let only_context = ModelLimits {
            context: Some(128_000),
            max_output: None,
            source: ModelLimitSource::Manual,
        };
        let text = only_context.display().expect("只有上下文也要能显示");
        assert!(text.contains("128K"), "实际：{text}");
        assert!(
            !text.contains(" · "),
            "只有一段时不该有分隔符，实际：{text}"
        );

        let empty = ModelLimits::default();
        assert!(empty.display().is_none(), "没填就要显示「未填」而不是 0");
        assert!(empty.is_empty());
    }

    #[test]
    fn provider_protocol_field_is_backward_compatible() {
        // 老 config.json 没有 protocol 字段：反序列化后应为 None（= 沿用旧行为）
        let legacy = r#"{
            "id": "relay",
            "name": "中转站",
            "base_url": "https://relay.example.com/v1",
            "api_key": { "kind": "env", "var": "RELAY_KEY" },
            "models": ["m1"]
        }"#;
        let parsed: Provider = serde_json::from_str(legacy).expect("老配置要能读");
        assert!(parsed.protocol.is_none());
        assert!(parsed.model_limits.is_empty());
        assert!(parsed.catalog_id.is_none());
    }

    #[test]
    fn claude_settings_merge_keeps_other_keys() {
        let original = "{\n  \"hooks\": {\"Stop\": []},\n  \"env\": {\"KEEP\": \"1\"}\n}\n";
        let p = provider(SecretRef::Env {
            var: "RELAY_KEY".into(),
        });
        let write = plain(&p, "claude-sonnet-4-5", true, Some("sk-test"));
        let merged = merge_claude_settings(Some(original), &write).unwrap();
        let doc: Value = serde_json::from_str(&merged).unwrap();
        assert_eq!(doc["hooks"]["Stop"], json!([]), "无关键必须保留");
        assert_eq!(doc["env"]["KEEP"], json!("1"), "已有 env 键必须保留");
        assert_eq!(
            doc["env"]["ANTHROPIC_BASE_URL"],
            json!("https://relay.example.com/v1")
        );
        assert_eq!(doc["env"]["ANTHROPIC_MODEL"], json!("claude-sonnet-4-5"));
        assert_eq!(doc["env"]["ANTHROPIC_AUTH_TOKEN"], json!("sk-test"));
        assert_eq!(doc["model"], json!("claude-sonnet-4-5"));
        assert!(merged.ends_with('\n'));
    }

    #[test]
    fn claude_settings_keeps_crlf_and_bom() {
        let original = "\u{feff}{\r\n  \"env\": {}\r\n}\r\n";
        let p = provider(SecretRef::Env { var: "K".into() });
        let write = plain(&p, "m", false, Some("v"));
        let merged = merge_claude_settings(Some(original), &write).unwrap();
        assert!(merged.starts_with('\u{feff}'), "BOM 必须写回");
        assert!(merged.contains("\r\n"), "CRLF 必须保留");
        assert!(!merged.contains("\n\n") || merged.contains("\r\n\r\n"));
        assert!(merged.ends_with("\r\n"));
        assert!(!merged.contains("\"model\""), "未设为默认时不动顶层 model");
    }

    #[test]
    fn claude_settings_rejects_non_object() {
        let p = provider(SecretRef::Env { var: "K".into() });
        let write = plain(&p, "m", false, Some("v"));
        assert!(merge_claude_settings(Some("[1,2,3]"), &write).is_err());
        assert!(merge_claude_settings(Some("{\"env\": 3}"), &write).is_err());
        assert!(merge_claude_settings(Some("not json"), &write).is_err());
    }

    #[test]
    fn codex_config_keeps_comments_and_other_providers() {
        let original = concat!(
            "# 我的 codex 配置\n",
            "model = \"gpt-6.1\"\n",
            "notify = [\"say\"]\n",
            "\n",
            "[model_providers.official]\n",
            "name = \"official\"\n",
            "base_url = \"https://api.openai.com/v1\"\n",
        );
        let p = provider(SecretRef::Env {
            var: "RELAY_API_KEY".into(),
        });
        let write = plain(&p, "gpt-6.1-sol", true, None);
        let merged = merge_codex_config(Some(original), &write).unwrap();
        assert!(merged.contains("# 我的 codex 配置"), "注释必须保留");
        assert!(merged.contains("[\"say\"]"), "无关键必须保留");
        assert!(
            merged.contains("[model_providers.official]"),
            "其它 provider 表必须保留"
        );
        assert!(merged.contains("[model_providers.relay]"));
        assert!(
            merged.contains("env_key = \"RELAY_API_KEY\""),
            "Codex 只写变量名"
        );
        assert!(merged.contains("model_provider = \"relay\""));
        let doc: DocumentMut = merged.parse().unwrap();
        assert_eq!(
            doc["model_providers"]["official"]["base_url"].as_str(),
            Some("https://api.openai.com/v1")
        );
    }

    #[test]
    fn codex_config_keeps_crlf() {
        let original = "model = \"a\"\r\n";
        let p = provider(SecretRef::Env { var: "K".into() });
        let write = plain(&p, "b", true, None);
        let merged = merge_codex_config(Some(original), &write).unwrap();
        assert!(merged.contains("\r\n"));
        assert!(!merged.replace("\r\n", "").contains('\n'), "不能混入裸 LF");
    }

    #[test]
    fn opencode_merges_only_declared_keys() {
        let original = concat!(
            "{\n",
            "  \"$schema\": \"https://opencode.ai/config.json\",\n",
            "  \"provider\": {\n",
            "    \"other\": { \"options\": { \"baseURL\": \"https://other.example.com\" } }\n",
            "  },\n",
            "  \"theme\": \"dark\"\n",
            "}\n"
        );
        let p = provider(SecretRef::Literal {
            value: "wk-test".into(),
            acknowledged: true,
        });
        let write = plain(&p, "some-model", true, Some("wk-test"));
        let spec = spec_for(&AgentId::Opencode, Some(Path::new("/tmp/opencode"))).unwrap();
        assert_eq!(spec.file, Path::new("/tmp/opencode").join("opencode.json"));

        let merged = merge_for(&spec, Some(original), &write).unwrap();
        let doc: Value = serde_json::from_str(&merged).unwrap();
        assert_eq!(
            doc["provider"]["relay"]["options"]["baseURL"],
            json!("https://relay.example.com/v1")
        );
        assert_eq!(
            doc["provider"]["relay"]["options"]["apiKey"],
            json!("wk-test")
        );
        assert_eq!(
            doc["provider"]["other"]["options"]["baseURL"],
            json!("https://other.example.com"),
            "别的 provider 必须原样保留"
        );
        assert_eq!(doc["$schema"], json!("https://opencode.ai/config.json"));
        assert_eq!(doc["theme"], json!("dark"));
        // 缺 npm / models 时 OpenCode 不会加载这个 provider，这是「写了不生效」的根因
        assert_eq!(
            doc["provider"]["relay"]["npm"],
            json!("@ai-sdk/openai-compatible")
        );
        assert_eq!(doc["provider"]["relay"]["name"], json!("中转站"));
        assert_eq!(
            doc["provider"]["relay"]["models"]["some-model"]["name"],
            json!("some-model")
        );
        assert!(
            doc.get("model").is_none(),
            "OpenCode 的模型键未核实，任何情况下都不该写"
        );
    }

    #[test]
    fn unsupported_agents_have_specific_reasons() {
        // 不断言具体语言：只要每个 agent 都有非空、且彼此不同的原因即可
        let mut seen = std::collections::BTreeSet::new();
        for agent in [AgentId::Gemini, AgentId::CodeBuddy, AgentId::Hermes] {
            assert!(!supports(&agent));
            let reason = unsupported_reason(&agent);
            assert!(
                !reason.trim().is_empty(),
                "{} 的原因不能为空",
                agent.display_name()
            );
            assert!(
                seen.insert(reason.clone()),
                "{} 的原因与别的 agent 重复：{reason}",
                agent.display_name()
            );
            assert!(spec_for(&agent, Some(Path::new("/tmp/x"))).is_none());
        }
    }

    #[test]
    fn target_path_follows_config_dir() {
        let dir = Path::new("/tmp/custom-claude");
        assert_eq!(
            target_path(&AgentId::Claude, Some(dir)).unwrap(),
            dir.join("settings.json")
        );
        assert_eq!(
            target_path(&AgentId::Codex, Some(dir)).unwrap(),
            dir.join("config.toml")
        );
        assert!(target_path(&AgentId::Gemini, Some(dir)).is_err());
        assert!(target_path(&AgentId::Claude, None).is_err());
    }
}
