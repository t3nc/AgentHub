//! 设置页的读写。
//!
//! 两条原则：
//! - **一次读全**：`SettingsView` 同时给出「当前生效值」与「是否存在」，
//!   前端不需要为每个字段再来一次 IPC。
//! - **一次写全**：`SettingsInput` 全量提交（避免 `Option<Option<T>>` 在 JSON 里
//!   无法区分「没传」与「传 null」），后端校验后整体替换并重建适配器。

use crate::l10n::{Msg, t};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::adapters::{AgentAdapter, display_path};
use crate::config::{AgentPathOverride, AppConfig, CustomAgentDef, MarketSource};
use crate::error::{Error, Result};
use crate::link::LinkStrategy;
use crate::model::agent::{AgentId, Scope};
use crate::model::gateway::{FeatureFlags, FeatureMeta, feature_catalog};
use crate::paths;

/// 某个 agent 的当前生效路径（已缩成 `~/…`）。
#[derive(Debug, Clone, Default, Serialize)]
pub struct EffectivePaths {
    pub config_dir: Option<String>,
    pub skills_dir: Option<String>,
    /// 主技能目录之外还会读的位置（OpenCode 的 `.agents/skills`、`.claude/skills`）。
    /// 这些是内置值，不可覆盖；扫描时与其它 agent 共享同一目录只归属一个。
    pub extra_skills_dirs: Vec<String>,
    pub mcp_file: Option<String>,
    pub instructions: Vec<String>,
}

/// 与 `EffectivePaths` 一一对应的「是否存在」。
#[derive(Debug, Clone, Default, Serialize)]
pub struct PathPresence {
    pub config_dir: bool,
    pub skills_dir: bool,
    pub extra_skills_dirs: Vec<bool>,
    pub mcp_file: bool,
    pub instructions: Vec<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentPathsView {
    /// agent slug（`hermes` / `custom:xxx`）
    pub id: String,
    pub display_name: String,
    pub builtin: bool,
    /// 用户在设置里填的覆盖（空字段 = 跟随自动检测）
    pub overrides: AgentPathOverride,
    pub effective: EffectivePaths,
    pub presence: PathPresence,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct StoreUsage {
    pub skills: usize,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SettingsView {
    pub data_root: String,
    pub store_root: String,
    pub store_usage: StoreUsage,
    /// `None` = 跟随平台默认
    pub default_strategy: Option<LinkStrategy>,
    /// 平台默认（Windows 目录联接 / 其它软链），UI 用来说明「跟随」的含义
    pub platform_strategy: LinkStrategy,
    pub backups_keep: usize,
    pub excluded_paths: Vec<String>,
    pub use_trash_for_dedupe: bool,
    pub backup_config_files: bool,
    pub agents: Vec<AgentPathsView>,
    pub custom_agents: Vec<CustomAgentDef>,
    /// Phase 3：provider 注册表与各 agent 的绑定状态（只读；改它走 provider 命令）
    pub providers: crate::api::provider::ProvidersView,
    /// 技能市场生效数据源（`skillhub` / `clawhub`）
    pub market_source: String,
    /// 数据源展示名，市场浮层徽标直接用
    pub market_source_label: String,
    /// 技能市场自定义源（None = 跟随默认）
    pub market_registry: Option<String>,
    /// 当前数据源的官方地址，UI 用作输入框占位符
    pub market_default_registry: String,
    /// 功能开关当前状态（即时生效，不进 `SettingsInput`）
    pub features: FeatureFlags,
    /// 功能管理一行的元数据（名称 / 说明 / 是否启用），顺序固定
    pub feature_catalog: Vec<FeatureMeta>,
    /// 界面语言的选择：`system` / `zh-Hans` / `zh-Hant` / `en`
    pub locale: String,
    /// 实际生效的语言代码（「跟随系统」时用它告诉用户当前是哪一种）
    pub locale_effective: String,
}

/// 保存时提交的结构：只有可写字段，展示字段（effective / presence）不回传。
#[derive(Debug, Clone, Deserialize)]
pub struct SettingsInput {
    pub store_root: String,
    #[serde(default)]
    pub default_strategy: Option<LinkStrategy>,
    pub backups_keep: usize,
    #[serde(default)]
    pub excluded_paths: Vec<String>,
    #[serde(default)]
    pub use_trash_for_dedupe: bool,
    #[serde(default = "default_true")]
    pub backup_config_files: bool,
    #[serde(default)]
    pub agent_paths: BTreeMap<String, AgentPathOverride>,
    #[serde(default)]
    pub custom_agents: Vec<CustomAgentDef>,
    /// 技能市场数据源（`skillhub` / `clawhub`）；空串 = 内置默认（老前端不传也算空）
    #[serde(default)]
    pub market_source: String,
    /// 技能市场源地址；空串 = 跟随内置默认（老前端不传也算空）
    #[serde(default)]
    pub market_registry: String,
}

fn default_true() -> bool {
    true
}

/// 读取：把适配器的生效路径与配置里的覆盖拼成一份视图。
pub fn view(config: &AppConfig, adapters: &[Box<dyn AgentAdapter>]) -> SettingsView {
    let store_root = config.store_root_path();
    let agents = adapters
        .iter()
        .map(|adapter| {
            let id = adapter.id();
            let slug = id.slug();
            let builtin = !matches!(id, AgentId::Custom(_));
            let overrides = config.agent_paths.get(&slug).cloned().unwrap_or_default();

            let config_dir = adapter.config_dir();
            let targets = adapter.skill_targets(&Scope::User);
            let skills_dir = targets.first().map(|target| target.dir.clone());
            let extra_skills: Vec<PathBuf> = targets
                .iter()
                .skip(1)
                .map(|target| target.dir.clone())
                .collect();
            let mcp_file = adapter
                .mcp_files(&Scope::User)
                .first()
                .map(|file| file.path.clone());
            let instructions: Vec<PathBuf> = adapter
                .instruction_specs(&Scope::User)
                .into_iter()
                .map(|spec| spec.path)
                .collect();

            let effective = EffectivePaths {
                config_dir: config_dir.as_deref().map(display_path),
                skills_dir: skills_dir.as_deref().map(display_path),
                extra_skills_dirs: extra_skills.iter().map(|p| display_path(p)).collect(),
                mcp_file: mcp_file.as_deref().map(display_path),
                instructions: instructions.iter().map(|p| display_path(p)).collect(),
            };
            let presence = PathPresence {
                config_dir: config_dir.as_ref().map(|p| p.exists()).unwrap_or(false),
                skills_dir: skills_dir.as_ref().map(|p| p.is_dir()).unwrap_or(false),
                extra_skills_dirs: extra_skills.iter().map(|p| p.is_dir()).collect(),
                mcp_file: mcp_file.as_ref().map(|p| p.exists()).unwrap_or(false),
                instructions: instructions.iter().map(|p| p.exists()).collect(),
            };

            // 自定义平台的 AgentId 存的是登记 key，展示名要回 config 里查，
            // 否则设置页会把「Trae」显示成「trae」
            let display_name = match &id {
                AgentId::Custom(key) => config
                    .custom_agents
                    .iter()
                    .find(|def| &def.id == key)
                    .map(|def| def.display_name.clone())
                    .unwrap_or_else(|| id.display_name()),
                _ => id.display_name(),
            };
            AgentPathsView {
                id: slug,
                display_name,
                builtin,
                overrides,
                effective,
                presence,
            }
        })
        .collect();

    // 生效数据源：自定义地址优先反推（老配置里可能只有 registry_url），
    // 解析失败（比如手改坏了配置）不让设置页整体读不出来，退回显式配置值
    let source = crate::market::target(config)
        .map(|target| target.source)
        .unwrap_or_else(|_| config.market.source());

    SettingsView {
        data_root: paths::path_str(&paths::data_root()),
        store_root: paths::path_str(&store_root),
        store_usage: store_usage(&store_root),
        default_strategy: config.default_strategy,
        platform_strategy: LinkStrategy::platform_default(),
        backups_keep: config.backups_keep,
        excluded_paths: config
            .excluded_paths
            .iter()
            .map(|p| paths::path_str(p))
            .collect(),
        use_trash_for_dedupe: config.adoption.use_trash_for_dedupe,
        backup_config_files: config.adoption.backup_config_files,
        agents,
        custom_agents: config.custom_agents.clone(),
        providers: crate::api::provider::view(config),
        market_source: source.key().to_string(),
        market_source_label: source.label().to_string(),
        market_registry: config.market.base_url(),
        market_default_registry: source.default_base().to_string(),
        features: config.features.clone(),
        feature_catalog: feature_catalog(&config.features),
        locale: config
            .locale
            .clone()
            .filter(|raw| !raw.trim().is_empty())
            .unwrap_or_else(|| "system".to_string()),
        locale_effective: crate::l10n::current().code().to_string(),
    }
}

fn store_usage(root: &Path) -> StoreUsage {
    let skills = std::fs::read_dir(crate::platform::long_path(root))
        .map(|read| {
            read.filter_map(|entry| entry.ok())
                .filter(|entry| entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false))
                .count()
        })
        .unwrap_or(0);
    let (bytes, _files) = paths::dir_usage(root);
    StoreUsage { skills, bytes }
}

fn normalize(raw: Option<&str>) -> Option<String> {
    raw.map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

/// 校验并构造新配置（纯函数，便于单测）。
pub fn build_config(current: &AppConfig, input: &SettingsInput) -> Result<AppConfig> {
    let store_raw = input.store_root.trim();
    if store_raw.is_empty() {
        return Err(Error::msg(t!(Msg::ErrStoreRootEmpty)));
    }
    if input.backups_keep == 0 || input.backups_keep > 100 {
        return Err(Error::msg(t!(Msg::ErrBackupKeepRange)));
    }

    let builtin_slugs: Vec<String> = AgentId::builtin().iter().map(|id| id.slug()).collect();

    let mut next = current.clone();
    next.store_root = paths::expand(store_raw);
    next.default_strategy = input.default_strategy;
    next.backups_keep = input.backups_keep;
    next.excluded_paths = input
        .excluded_paths
        .iter()
        .filter_map(|raw| normalize(Some(raw)))
        .map(|raw| paths::expand(&raw))
        .collect();
    next.adoption.use_trash_for_dedupe = input.use_trash_for_dedupe;
    next.adoption.backup_config_files = input.backup_config_files;

    let mut seen: HashSet<&str> = HashSet::new();
    for def in &input.custom_agents {
        let id = def.id.trim();
        if id.is_empty() {
            return Err(Error::msg(t!(Msg::ErrCustomAgentIdEmpty)));
        }
        if id.starts_with("custom:") {
            return Err(Error::msg(t!(Msg::ErrCustomAgentIdPrefix, id = id)));
        }
        if builtin_slugs.iter().any(|slug| slug == id) {
            return Err(Error::msg(t!(Msg::ErrCustomAgentIdConflict, id = id)));
        }
        if !seen.insert(id) {
            return Err(Error::msg(t!(Msg::ErrCustomAgentIdDuplicate, id = id)));
        }
        let has_target = def.skills_dir.is_some()
            || def.project_skills_dir.is_some()
            || def.mcp_file.is_some()
            || def.project_mcp_file.is_some()
            || !def.instructions.is_empty();
        if !has_target {
            return Err(Error::msg(t!(Msg::ErrCustomAgentNoTarget, id = id)));
        }
    }
    next.custom_agents = input
        .custom_agents
        .iter()
        .map(|def| {
            let id = def.id.trim().to_string();
            let display = def.display_name.trim();
            CustomAgentDef {
                id: id.clone(),
                display_name: if display.is_empty() {
                    id
                } else {
                    display.to_string()
                },
                ..def.clone()
            }
        })
        .collect();

    let mut paths_next: BTreeMap<String, AgentPathOverride> = BTreeMap::new();
    for (slug, overrides) in &input.agent_paths {
        if !builtin_slugs.iter().any(|known| known == slug) {
            return Err(Error::msg(t!(Msg::ErrUnknownBuiltinAgent, agent = slug)));
        }
        let normalized = AgentPathOverride {
            config_dir: normalize(overrides.config_dir.as_deref()),
            skills_dir: normalize(overrides.skills_dir.as_deref()),
            mcp_file: normalize(overrides.mcp_file.as_deref()),
            instructions: overrides
                .instructions
                .iter()
                .filter_map(|raw| normalize(Some(raw)))
                .collect(),
        };
        if !normalized.is_empty() {
            paths_next.insert(slug.clone(), normalized);
        }
    }
    next.agent_paths = paths_next;

    // 市场数据源：空 = 跟随内置默认；非法值直接拒绝，别把错字写进配置
    let source = match normalize(Some(&input.market_source)) {
        Some(raw) => MarketSource::parse(&raw)
            .ok_or_else(|| Error::msg(t!(Msg::ErrUnknownMarketSource, value = raw)))?,
        None => MarketSource::default(),
    };
    next.market.source = Some(source.key().to_string());

    // 市场源：空 = 跟随默认；非空必须是 https，避免把明文 http 或其垃圾串写进配置
    let registry = normalize(Some(&input.market_registry));
    if let Some(raw) = &registry {
        crate::market::normalize_base(raw)?;
    }
    next.market.registry_url = registry;

    Ok(next)
}
