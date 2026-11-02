use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::ext::pricing::ModelPrice;
use crate::ext::provider::{Provider, ProviderBinding};
use crate::link::LinkStrategy;
use crate::model::gateway::{FeatureFlags, GatewayConfig, GatewayRoute};
use crate::model::i18n::TranslationConsent;
use crate::model::mcp::McpFormat;
use crate::paths;

fn default_store_root() -> PathBuf {
    paths::default_store_root()
}

fn default_keep() -> usize {
    10
}

/// serde 默认值：缺字段一律「开启 / 接受」，老配置因此保持既有行为。
pub(crate) fn default_true() -> bool {
    true
}

/// agent 独有技能的出厂白名单（按技能名）。
///
/// 这些技能在不同 agent 下的实现互不相同（`computer-use` 依赖各家自己的截图 /
/// 输入通道），收敛成「一份真身 + 多条链接」只会把别人的实现塞给另一家。
/// 因此出厂就排除在统一管理之外，用户可右键增删。
const DEFAULT_AGENT_LOCAL_SKILLS: [&str; 6] = [
    "computer-use",
    "computer_use",
    "computeruse",
    "computer-control",
    "browser-use",
    "browser_use",
];

fn default_agent_local_skills() -> Vec<String> {
    DEFAULT_AGENT_LOCAL_SKILLS
        .iter()
        .map(|name| name.to_string())
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdoptionPolicy {
    /// 去重时把旧副本移入系统回收站（可撤销）。
    /// 默认 false：走 AgentHub 自己的备份目录，避免无 GUI 会话下 Finder/AppleEvent 超时。
    #[serde(default)]
    pub use_trash_for_dedupe: bool,
    /// 改写配置文件前先备份
    #[serde(default = "default_true")]
    pub backup_config_files: bool,
}

impl Default for AdoptionPolicy {
    fn default() -> Self {
        Self {
            use_trash_for_dedupe: false,
            backup_config_files: true,
        }
    }
}

/// 技能市场数据源。
///
/// 两家的读接口协议并不通用（端点集合、参数名、响应信封都不同），
/// 所以换源不是改一个 URL，而是换一个 [`crate::market::Registry`] 适配器。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarketSource {
    /// 腾讯 SkillHub（api.skillhub.cn）：国内直连，默认源
    #[default]
    SkillHub,
    /// ClawHub（clawhub.ai）：海外源，保留作为可选回退
    ClawHub,
}

impl MarketSource {
    pub const ALL: [MarketSource; 2] = [MarketSource::SkillHub, MarketSource::ClawHub];

    pub fn key(&self) -> &'static str {
        match self {
            MarketSource::SkillHub => "skillhub",
            MarketSource::ClawHub => "clawhub",
        }
    }

    /// 展示名（设置页下拉与市场浮层徽标用）。
    pub fn label(&self) -> &'static str {
        match self {
            MarketSource::SkillHub => "SkillHub",
            MarketSource::ClawHub => "ClawHub",
        }
    }

    /// 该源的官方接口地址（注意 SkillHub 的接口在 `api.` 子域，不是产品站）。
    pub fn default_base(&self) -> &'static str {
        match self {
            MarketSource::SkillHub => "https://api.skillhub.cn",
            MarketSource::ClawHub => "https://clawhub.ai",
        }
    }

    /// 宽容解析：非法值返回 None，由调用方退默认，不让一个错字把配置读崩。
    pub fn parse(raw: &str) -> Option<Self> {
        let key = raw.trim().to_ascii_lowercase();
        Self::ALL.into_iter().find(|source| source.key() == key)
    }
}

/// 技能市场配置。
///
/// - `source`：选哪个数据源，空/非法 = 默认 SkillHub（老配置没有这个字段）；
/// - `registry_url`：自定义源地址，空 = 跟随所选源的官方地址。
///
/// 老配置里可能只填了 `registry_url = "https://clawhub.ai"`，
/// 解析时会按域名把数据源反推回 ClawHub，升级不会失效。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MarketConfig {
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub registry_url: Option<String>,
}

impl MarketConfig {
    /// 生效的源地址（已做去空格与去尾斜杠；非法值在调用处报错）。
    pub fn base_url(&self) -> Option<String> {
        self.registry_url
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.trim_end_matches('/').to_string())
    }

    /// 生效的数据源（无法识别时退默认 SkillHub）。
    pub fn source(&self) -> MarketSource {
        self.source
            .as_deref()
            .and_then(MarketSource::parse)
            .unwrap_or_default()
    }
}

/// 单个内置 agent 的路径覆盖（key = agent slug，如 `hermes`）。
///
/// 四个字段都可空：空 = 跟随自动检测。存的是可移植原文（`~/…`、`%LOCALAPPDATA%/…`），
/// 由 `paths::expand` 在运行时解析，换机器仍然有效。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentPathOverride {
    #[serde(default)]
    pub config_dir: Option<String>,
    #[serde(default)]
    pub skills_dir: Option<String>,
    #[serde(default)]
    pub mcp_file: Option<String>,
    /// 指令文件列表；空 = 用适配器默认。非空时完全替换（不按位置合并）。
    #[serde(default)]
    pub instructions: Vec<String>,
}

impl AgentPathOverride {
    /// 是否四项都为空 —— 空覆盖等价于「没设置」，可以省掉一条配置。
    pub fn is_empty(&self) -> bool {
        self.config_dir.is_none()
            && self.skills_dir.is_none()
            && self.mcp_file.is_none()
            && self.instructions.is_empty()
    }
}

/// 用户自定义平台：本机未内置的 agent（如 DeepSeek Harness）由用户填路径接入。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomAgentDef {
    pub id: String,
    pub display_name: String,
    #[serde(default)]
    pub skills_dir: Option<String>,
    #[serde(default)]
    pub project_skills_dir: Option<String>,
    #[serde(default)]
    pub mcp_file: Option<String>,
    /// 项目级 MCP 文件（相对项目根的路径，如 Trae 的 `.trae/mcp.json`）。
    /// 只有「配置写在项目里」的产品才用得上；用户级 MCP 仍走 `mcp_file`。
    #[serde(default)]
    pub project_mcp_file: Option<String>,
    #[serde(default)]
    pub mcp_format: Option<McpFormat>,
    #[serde(default)]
    pub instructions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_store_root")]
    pub store_root: PathBuf,
    /// 默认链接策略；None 表示按平台自动（mac/Linux 软链，Windows junction）
    #[serde(default)]
    pub default_strategy: Option<LinkStrategy>,
    /// 单条技能的链接策略覆盖（key = 技能名，也就是中央仓库里的目录名）。
    /// 「一份真身 + 多条链接」的模型下，同一技能在所有 agent 下共用一条覆盖。
    #[serde(default)]
    pub skill_strategy_overrides: BTreeMap<String, LinkStrategy>,
    #[serde(default = "default_keep")]
    pub backups_keep: usize,
    /// 这些目录下的技能不参与上传与翻译（Phase 5）
    #[serde(default)]
    pub excluded_paths: Vec<PathBuf>,
    /// agent 独有技能白名单（按技能名，大小写不敏感）。
    ///
    /// 名单里的技能不参与统一管理：不去重接管、不被链接修复改写、不进重复组与
    /// 浪费空间统计。出厂带一份 [`DEFAULT_AGENT_LOCAL_SKILLS`]，用户可右键增删。
    ///
    /// 用 `serde(default = …)` 而不是 `Vec::new()`：老配置没有这个字段时也要拿到
    /// 默认名单；用户显式清空后序列化成 `[]`，反序列化仍是空，不会被默认值回填。
    #[serde(default = "default_agent_local_skills")]
    pub agent_local_skills: Vec<String>,
    #[serde(default)]
    pub custom_agents: Vec<CustomAgentDef>,
    /// 内置 agent 的路径覆盖（key = agent slug）。`~/.hermes` 为空、
    /// 实际数据在 `%LOCALAPPDATA%\hermes` 这类情况由用户在这里纠正。
    #[serde(default)]
    pub agent_paths: BTreeMap<String, AgentPathOverride>,
    #[serde(default)]
    pub recent_projects: Vec<PathBuf>,
    #[serde(default)]
    pub translation: TranslationConsent,
    #[serde(default)]
    pub adoption: AdoptionPolicy,
    /// Phase 3：登记过的 provider（中转站 / 官方 API），密钥只存 `SecretRef` 引用。
    #[serde(default)]
    pub providers: Vec<Provider>,
    /// 各 agent 当前绑定的 provider（key = agent slug）。
    ///
    /// 独立于 `SettingsInput` 全量保存：`build_config` 是 `current.clone()` 后只覆盖
    /// 白名单字段，所以这里不会被「保存设置」清空（有回归测试锁住）。
    #[serde(default)]
    pub provider_bindings: BTreeMap<String, ProviderBinding>,
    /// 技能市场数据源（空 = 内置默认 ClawHub）
    #[serde(default)]
    pub market: MarketConfig,
    /// Phase 4：模型价格登记（每 100 万 token 单价）。
    ///
    /// 属于配置登记而不是对 agent 目录的改写，所以不进 ChangeSet；用户手填或从
    /// 公开源导入后立即写 `config.json`，与 `providers` 同语义。
    #[serde(default)]
    pub usage_pricing: Vec<ModelPrice>,
    /// 功能开关（设置 › 功能管理）。缺字段一律视为开启。
    ///
    /// **不进 `SettingsInput`**：它是即时生效的独立开关，与「保存并重扫」不是一条
    /// 链路，混进去会被全量提交覆盖。
    #[serde(default)]
    pub features: FeatureFlags,
    /// 界面语言：`None` / `"system"` = 跟随系统，其余为 `zh-Hans` / `zh-Hant` / `en`。
    ///
    /// 与 `features` 同理**不进 `SettingsInput`**：切换语言要即时生效，
    /// 走独立的 `set_locale` 命令直接落盘，避免被「保存并重扫」的全量提交覆盖。
    /// GUI 与 CLI 共用这个字段，所以权威值放在配置里而不是 localStorage。
    #[serde(default)]
    pub locale: Option<String>,
    /// 本地网关进程级配置。
    #[serde(default)]
    pub gateway: GatewayConfig,
    /// 网关路由。删除 provider 时同步清理引用（见 `api::gateway`）。
    #[serde(default)]
    pub gateway_routes: Vec<GatewayRoute>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            store_root: default_store_root(),
            default_strategy: None,
            skill_strategy_overrides: BTreeMap::new(),
            backups_keep: default_keep(),
            excluded_paths: Vec::new(),
            agent_local_skills: default_agent_local_skills(),
            custom_agents: Vec::new(),
            agent_paths: BTreeMap::new(),
            recent_projects: Vec::new(),
            translation: TranslationConsent::default(),
            adoption: AdoptionPolicy::default(),
            providers: Vec::new(),
            provider_bindings: BTreeMap::new(),
            market: MarketConfig::default(),
            usage_pricing: Vec::new(),
            features: FeatureFlags::default(),
            locale: None,
            gateway: GatewayConfig::default(),
            gateway_routes: Vec::new(),
        }
    }
}

impl AppConfig {
    /// 读配置；文件不存在或损坏都退化为默认值，不阻塞启动。
    pub fn load() -> Self {
        Self::load_from(&paths::config_path())
    }

    pub fn load_from(path: &PathBuf) -> Self {
        match std::fs::read_to_string(path) {
            // 用户可能用记事本改过 config.json，带 BOM 的 JSON 会解析失败并静默退化默认值
            Ok(text) => serde_json::from_str(crate::text::strip_bom(&text)).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = paths::config_path();
        if let Some(parent) = path.parent() {
            paths::ensure_dir(parent)?;
        }
        let text = serde_json::to_string_pretty(self)?;
        paths::write_atomic(&path, text.as_bytes())
    }

    pub fn store_root_path(&self) -> PathBuf {
        paths::expand(&self.store_root.to_string_lossy())
    }

    pub fn strategy_default(&self) -> LinkStrategy {
        LinkStrategy::resolve(None, self.default_strategy)
    }

    /// 某条技能的覆盖值；没有覆盖返回 None（表示跟随全局默认 / 平台默认）。
    pub fn skill_strategy(&self, name: &str) -> Option<LinkStrategy> {
        self.skill_strategy_overrides.get(name).copied()
    }

    /// 设置或清除单条技能的覆盖。`None` = 清除，回到「跟随全局 / 平台默认」。
    /// 返回解析后的生效策略，供调用方回显。
    pub fn set_skill_strategy(
        &mut self,
        name: &str,
        strategy: Option<LinkStrategy>,
    ) -> LinkStrategy {
        let name = name.trim();
        if name.is_empty() {
            return self.strategy_default();
        }
        match strategy {
            Some(value) => {
                self.skill_strategy_overrides
                    .insert(name.to_string(), value);
                LinkStrategy::resolve(Some(value), self.default_strategy)
            }
            None => {
                self.skill_strategy_overrides.remove(name);
                LinkStrategy::resolve(None, self.default_strategy)
            }
        }
    }

    pub fn remember_project(&mut self, root: &std::path::Path) {
        let normalized = paths::normalize_lexical(root);
        // Windows 上同一项目可能以不同大小写被记录，按平台等价语义去重
        self.recent_projects
            .retain(|p| !paths::paths_eq(p, &normalized));
        self.recent_projects.insert(0, normalized);
        self.recent_projects.truncate(12);
    }

    pub fn is_excluded(&self, path: &std::path::Path) -> bool {
        self.excluded_paths
            .iter()
            .any(|ex| paths::is_within(&paths::expand(&ex.to_string_lossy()), path))
    }

    /// 该技能名是否被标记为「agent 独有」（不参与统一管理）。
    ///
    /// 比较用 `eq_ignore_ascii_case`：名单可能是用户手改 config 填进去的，
    /// Windows 目录名大小写也不敏感，不能因为 `Computer-Use` 就漏判。
    pub fn is_agent_local(&self, name: &str) -> bool {
        let name = name.trim();
        !name.is_empty()
            && self
                .agent_local_skills
                .iter()
                .any(|known| known.trim().eq_ignore_ascii_case(name))
    }

    /// 标记 / 取消标记。返回归一化（trim + 小写、去重、排序）后的完整名单。
    ///
    /// 归一化只在写入侧做一次，后续比较仍然大小写不敏感，手改配置也不会失效。
    pub fn set_agent_local(&mut self, name: &str, on: bool) -> Vec<String> {
        let name = name.trim();
        if !name.is_empty() {
            let key = name.to_ascii_lowercase();
            self.agent_local_skills
                .retain(|known| !known.trim().eq_ignore_ascii_case(&key));
            if on {
                self.agent_local_skills.push(key);
            }
            // 缓存 key：名单很短，但 sort_by_key 会在每次比较时重新分配字符串
            self.agent_local_skills
                .sort_by_cached_key(|known| known.to_ascii_lowercase());
            self.agent_local_skills
                .dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        }
        self.agent_local_skills.clone()
    }
}
