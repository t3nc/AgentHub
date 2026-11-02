pub mod execute;
pub mod gateway;
pub mod market;
pub mod plan;
pub mod pricing;
pub mod provider;
pub mod settings;
pub mod usage;

use std::path::{Path, PathBuf};

use crate::adapters::{AgentAdapter, build_adapters};
use crate::apply::DoctorIssue;
use crate::config::AppConfig;
use crate::error::{Error, Result};
use crate::ext::catalog::{self, CatalogEntry};
use crate::l10n::{self, Msg, t};
use crate::link::LinkStrategy;
use crate::model::agent::{AgentId, AgentInstall, Capabilities, Scope, SkillTarget};
use crate::model::gateway::{FeatureFlags, FeatureKey, FeatureMeta, feature_catalog};
use crate::model::instruction::{ChainEntry, InstructionSpec};
use crate::model::mcp::McpFile;
use crate::model::skill::{LinkState, SkillRecord};
use crate::paths;
use crate::project::{self, ProjectSummary};
use crate::scan::{ScanReport, scan};
use crate::store::Db;
use crate::watch;

/// 清掉与内置 agent 同 id 的自定义平台（如 OpenCode 从「常见目录」升级为内置后的遗留）。
fn drop_shadowed_custom_agents(config: &mut AppConfig) -> bool {
    let builtin: Vec<String> = AgentId::builtin().iter().map(|id| id.slug()).collect();
    let before = config.custom_agents.len();
    config
        .custom_agents
        .retain(|def| !builtin.iter().any(|slug| slug == def.id.trim()));
    config.custom_agents.len() != before
}

/// 界面语言视图：设置页显示「选了哪个」与「实际生效哪个」。
#[derive(Debug, Clone, serde::Serialize)]
pub struct LocaleView {
    /// 用户选择：`system` / `zh-Hans` / `zh-Hant` / `en`
    pub mode: String,
    /// 实际生效的语言代码
    pub effective: String,
    /// 实际生效语言的 BCP-47 标签（`<html lang>` / `Intl` 用）
    pub tag: String,
}

/// 对外唯一门面：CLI 与 Tauri 命令层都只跟它打交道。
pub struct AgentHub {
    pub config: AppConfig,
    pub db: Db,
    adapters: Vec<Box<dyn AgentAdapter>>,
}

impl AgentHub {
    pub fn bootstrap() -> Result<Self> {
        Self::open(AppConfig::load())
    }

    pub fn open(mut config: AppConfig) -> Result<Self> {
        paths::ensure_dir(&paths::data_root())?;
        paths::ensure_dir(&config.store_root_path())?;
        // 旧版本里 OpenCode 是「常见目录」，可能已被登记成自定义平台；
        // 现在它是内置 agent，同 id 的自定义条目会让设置页校验失败，这里顺手清掉。
        if drop_shadowed_custom_agents(&mut config) {
            let _ = config.save();
        }
        // 语言在建门面时就位：此后任何一次取词（扫描汇总、计划标题）都是对的语言。
        // GUI 起来后会用自己的 `navigator.languages` 再校准一次（`set_locale`）。
        l10n::set_current(l10n::resolve(None, config.locale.as_deref()));
        let db = Db::open(&paths::db_path())?;
        let adapters = build_adapters(&config);
        Ok(Self {
            config,
            db,
            adapters,
        })
    }

    pub fn save_config(&self) -> Result<()> {
        self.config.save()
    }

    // ------------------------------------------------------------------
    // 功能开关（设置 › 功能管理）
    // ------------------------------------------------------------------

    pub fn features(&self) -> FeatureFlags {
        self.config.features.clone()
    }

    pub fn feature_catalog(&self) -> Vec<FeatureMeta> {
        feature_catalog(&self.config.features)
    }

    pub fn feature_enabled(&self, key: FeatureKey) -> bool {
        self.config.features.enabled(key)
    }

    /// 门禁：功能关闭时所有相关命令返回明确错误，而不是「静默空数据」。
    ///
    /// 前端已经把入口藏掉了，这里是第二道闸 —— CLI 与误调用的 IPC 都会撞上。
    pub fn require_feature(&self, key: FeatureKey) -> Result<()> {
        if self.config.features.enabled(key) {
            return Ok(());
        }
        Err(Error::Unsupported(t!(
            Msg::FeatureDisabled,
            name = key.display_name()
        )))
    }

    /// 翻转一个功能开关并立即写 `config.json`。
    ///
    /// 不进 ChangeSet：它不碰任何 agent 目录，只改本应用自己的配置，与
    /// `providers` / `usage_pricing` 同语义。禁用也是「隐藏入口」，数据一律保留。
    pub fn set_feature(&mut self, raw_key: &str, enabled: bool) -> Result<FeatureFlags> {
        let key = FeatureKey::parse(raw_key)
            .ok_or_else(|| Error::msg(t!(Msg::ErrUnknownFeature, key = raw_key)))?;
        if self.config.features.enabled(key) == enabled {
            return Ok(self.features());
        }
        self.config.features.set(key, enabled);
        self.config.save()?;
        Ok(self.features())
    }

    // ------------------------------------------------------------------
    // 界面语言
    // ------------------------------------------------------------------

    /// 配置里的语言选择：`system` 表示跟随系统。
    pub fn locale_mode(&self) -> String {
        self.config
            .locale
            .clone()
            .filter(|raw| !raw.trim().is_empty())
            .unwrap_or_else(|| "system".to_string())
    }

    /// 当前生效的界面语言视图。
    pub fn locale_view(&self) -> LocaleView {
        let locale = l10n::current();
        LocaleView {
            mode: self.locale_mode(),
            effective: locale.code().to_string(),
            tag: locale.tag().to_string(),
        }
    }

    /// 切换界面语言：校验 → 落 `config.json` → 更新进程级当前语言。
    ///
    /// - `effective` 是调用方已经解析好的实际语言（GUI 用 webview 的
    ///   `navigator.languages`，比读环境变量更贴近系统设置）；`None` 时按
    ///   环境变量推断，供 CLI 使用。
    /// - 与 [`Self::set_feature`] 同语义：只改本应用自己的配置，不进 ChangeSet、
    ///   不碰任何 agent 目录，所以不需要「保存并重扫」。
    pub fn set_locale(&mut self, mode_raw: &str, effective: Option<&str>) -> Result<LocaleView> {
        let raw = mode_raw.trim();
        let mode = if raw.is_empty() || raw.eq_ignore_ascii_case("system") {
            "system".to_string()
        } else {
            l10n::Locale::parse(raw)
                .ok_or_else(|| Error::msg(t!(Msg::ErrUnknownLocale, value = raw)))?
                .code()
                .to_string()
        };
        let resolved = match (mode.as_str(), effective.and_then(l10n::Locale::parse)) {
            ("system", Some(locale)) => locale,
            ("system", None) => l10n::resolve(None, Some("system")),
            (_, Some(locale)) => locale,
            (_, None) => l10n::resolve(None, Some(&mode)),
        };
        l10n::set_current(resolved);
        if self.config.locale.as_deref() != Some(mode.as_str()) {
            self.config.locale = Some(mode);
            self.config.save()?;
        }
        Ok(self.locale_view())
    }

    pub fn agents(&self) -> Vec<AgentInstall> {
        self.adapters.iter().map(|a| a.detect()).collect()
    }

    /// 常见 Agent 目录：内置 5 个 + 表里其余产品。
    ///
    /// `probe_versions = false` 时零子进程，可以随页面刷新随便调；
    /// 版本探测是用户显式动作，单条 1.2 s 上限。
    pub fn agent_catalog(&self, probe_versions: bool) -> Vec<CatalogEntry> {
        catalog::detect(&self.adapters, &self.config, probe_versions)
    }

    /// 把目录里的产品登记为受管平台。
    ///
    /// 只写 `config.json` —— 不创建技能目录、不建链接、不进入 ChangeSet；
    /// 真正的文件改动仍然要用户之后生成计划并确认执行。
    pub fn add_managed_agent(&mut self, key: &str) -> Result<Vec<CatalogEntry>> {
        let def = catalog::managed_def(key)?;
        if !self.config.custom_agents.iter().any(|d| d.id == def.id) {
            self.config.custom_agents.push(def);
            self.config.save()?;
            self.refresh_adapters();
        }
        Ok(self.agent_catalog(false))
    }

    /// 移出受管：只删配置里的登记。磁盘上的技能目录、已建链接一律不动。
    pub fn remove_managed_agent(&mut self, key: &str) -> Result<Vec<CatalogEntry>> {
        let id = catalog::managed_id(key)?;
        let before = self.config.custom_agents.len();
        self.config.custom_agents.retain(|d| d.id != id);
        if self.config.custom_agents.len() != before {
            self.config.save()?;
            self.refresh_adapters();
        }
        Ok(self.agent_catalog(false))
    }

    /// 自定义平台在 `open()` 之后才登记，适配器必须重建才能进入扫描/MCP 分发。
    fn refresh_adapters(&mut self) {
        self.adapters = build_adapters(&self.config);
    }

    /// 设置页的读取：路径生效值、仓库、策略、自定义平台。
    pub fn settings_view(&self) -> settings::SettingsView {
        settings::view(&self.config, &self.adapters)
    }

    /// 全量保存设置。
    ///
    /// 只写 `~/.agenthub/config.json`，然后重建适配器 —— 路径覆盖与自定义平台
    /// 都是「装配期」生效的，不重建就看不到变化（和「纳入受管」同一个坑）。
    pub fn save_settings(
        &mut self,
        input: settings::SettingsInput,
    ) -> Result<settings::SettingsView> {
        let next = settings::build_config(&self.config, &input)?;
        // 仓库目录换了就先建出来，否则后续扫描/备份会直接失败
        paths::ensure_dir(&next.store_root_path())?;
        self.config = next;
        self.config.save()?;
        self.refresh_adapters();
        Ok(self.settings_view())
    }

    pub fn adapter(&self, id: &AgentId) -> Option<&dyn AgentAdapter> {
        self.adapters
            .iter()
            .find(|a| &a.id() == id)
            .map(|a| a.as_ref())
    }

    pub fn capabilities(&self, id: &AgentId) -> Option<Capabilities> {
        self.adapter(id).map(|a| a.capabilities())
    }

    pub fn skill_targets(&self, scope: &Scope) -> Vec<SkillTarget> {
        self.adapters
            .iter()
            .flat_map(|a| a.skill_targets(scope))
            .collect()
    }

    pub fn mcp_files(&self, scope: &Scope) -> Vec<McpFile> {
        self.adapters
            .iter()
            .flat_map(|a| a.mcp_files(scope))
            .collect()
    }

    /// 允许写入的根目录集合。所有 apply 操作先过这道门。
    pub fn roots(&self, scope: &Scope) -> Vec<PathBuf> {
        let mut roots = vec![self.config.store_root_path(), paths::data_root()];
        for adapter in &self.adapters {
            if let Some(dir) = adapter.config_dir() {
                roots.push(dir);
            }
            for target in adapter.skill_targets(scope) {
                roots.push(target.dir);
            }
            // 只把「文件本身」列入可写根，而不是它的父目录。
            // 否则 `~/.claude.json`（位于用户主目录根）会把整个主目录变成受管根，
            // 写白名单被放大到 `$HOME` 全局——Windows 与 macOS 上都一样。
            for file in adapter.mcp_files(scope) {
                roots.push(file.path.clone());
            }
            for spec in adapter.instruction_specs(scope) {
                roots.push(spec.path.clone());
            }
        }
        if let Scope::Project { root } = scope {
            roots.push(root.clone());
        }
        for p in &self.config.recent_projects {
            roots.push(p.clone());
        }
        roots
    }

    /// 某条技能实际使用的链接策略。
    ///
    /// 解析顺序固定：单条覆盖 > 全局默认 > 平台默认。`agent` 目前不参与决策，
    /// 保留参数是为了将来支持「同一技能在不同 agent 下用不同策略」。
    pub fn strategy_for_skill(&self, name: &str, agent: &AgentId) -> LinkStrategy {
        let _ = agent;
        LinkStrategy::resolve(
            self.config.skill_strategy(name),
            self.config.default_strategy,
        )
    }

    /// 无技能名场景（如按 agent 批量建链）的薄封装。
    pub fn strategy_for(&self, agent: &AgentId) -> LinkStrategy {
        self.strategy_for_skill("", agent)
    }

    /// 记录/清除单条技能的链接策略，落盘后返回生效值。
    pub fn set_skill_strategy(
        &mut self,
        name: &str,
        strategy: Option<LinkStrategy>,
    ) -> Result<LinkStrategy> {
        let effective = self.config.set_skill_strategy(name, strategy);
        self.config.save()?;
        Ok(effective)
    }

    /// 标记 / 取消「agent 独有技能」。
    ///
    /// 只写 `config.json`：不改磁盘上的任何技能目录，**已存在的指向仓库的链接
    /// 也不会被拆掉**（那需要用户自己生成计划）。返回归一化后的完整名单，
    /// 前端拿到后应重扫一次 —— 标记来自扫描结果，只改本地 state 会不一致。
    pub fn set_agent_local_skill(&mut self, name: &str, on: bool) -> Result<Vec<String>> {
        let list = self.config.set_agent_local(name, on);
        self.config.save()?;
        Ok(list)
    }

    pub fn scan(&self, scope: &Scope) -> Result<ScanReport> {
        scan(&self.adapters, &self.config, &self.db, scope)
    }

    pub fn instructions_overview(&self, scope: &Scope) -> (Vec<InstructionSpec>, Vec<ChainEntry>) {
        (
            crate::instructions::collect(&self.adapters, scope),
            crate::instructions::chain(&self.adapters, scope),
        )
    }

    pub fn projects(&self) -> Vec<ProjectSummary> {
        self.config
            .recent_projects
            .iter()
            .map(|root| {
                let mut summary = project::summarize(root);
                summary.agent_dirs = self
                    .adapters
                    .iter()
                    .flat_map(|a| a.skill_targets(&Scope::Project { root: root.clone() }))
                    .map(|t| t.dir)
                    .collect();
                summary
            })
            .collect()
    }

    pub fn remember_project(&mut self, root: &Path) -> ProjectSummary {
        let root = paths::normalize_lexical(root);
        self.config.remember_project(&root);
        let _ = self.config.save();
        project::summarize(&root)
    }

    /// 需要监听外部变化的目录：中央仓库、各已检出 agent 的技能目录与配置目录。
    ///
    /// 只给一级目录，不做递归（理由见 `watch` 模块文档）。
    ///
    /// **数据根（`paths::data_root()`）刻意不在列表里**：每次扫描都会写索引库
    /// （`index.db` + WAL/SHM）、`apply` 会写批次 manifest，全都在数据根下。
    /// 监听它等于把自己的写盘当成「外部变更」，用户点「重扫」→ 扫描写库 → 又收到事件 →
    /// 提示条永远消不掉（真机踩过）。数据根里唯一需要感知的是 `config.json`，
    /// 而它只由本应用自己写（设置页保存后前端已经主动重扫）。
    /// 外部变更监听的目标。
    ///
    /// 技能目录与中央仓库关注全部一级子项；agent 配置目录只认适配器给出的配置文件
    /// （`config_watch_files`）。后者不能整目录监听——`~/.claude` / `~/.codex` 里
    /// 有 `history.jsonl`、`logs_2.sqlite-wal` 这类 CLI 运行时文件，常驻 agent 每写
    /// 一次就会推一条「外部有变更」，用户点了重扫过几秒又被顶回来。
    pub fn watch_targets(&self) -> Vec<watch::WatchRoot> {
        let mut out = vec![watch::WatchRoot::all(self.config.store_root_path())];
        for adapter in &self.adapters {
            for target in adapter.skill_targets(&Scope::User) {
                out.push(watch::WatchRoot::all(target.dir));
            }
            // 配置文件（settings.json / config.toml / mcp.json …）不落在 skills 目录里，
            // 不把它们的父目录加进来，Provider / MCP 的外部改动就永远看不见。
            if let Some(dir) = adapter.config_dir() {
                let names: Vec<String> = adapter
                    .config_watch_files()
                    .into_iter()
                    .filter_map(|path| {
                        path.file_name()
                            .map(|name| name.to_string_lossy().to_string())
                    })
                    .collect();
                // 一个名字都没有的配置根监听不到任何东西，不进列表省一次句柄
                if !names.is_empty() {
                    out.push(watch::WatchRoot::whitelist(dir, names));
                }
            }
        }
        out.retain(|root| root.path.exists());
        out.sort_by_key(|root| paths::path_key(&root.path));
        // 两个 agent 共用配置目录（如 Codex 与它的插件）时合并白名单，
        // 任一方是「全部关注」就按全部关注处理
        let mut merged: Vec<watch::WatchRoot> = Vec::with_capacity(out.len());
        for root in out {
            match merged.last_mut() {
                Some(prev) if paths::path_key(&prev.path) == paths::path_key(&root.path) => {
                    prev.merge(root.only);
                }
                _ => merged.push(root),
            }
        }
        merged
    }

    /// 体检：破损链接、未接管副本、仓库孤立项、记录与磁盘不一致。
    pub fn doctor(&self) -> Result<Vec<DoctorIssue>> {
        let report = self.scan(&Scope::User)?;
        Ok(self.doctor_from(&report))
    }

    /// 用已有的扫描结果做体检，避免「打开总览扫两遍」。
    pub fn doctor_from(&self, report: &ScanReport) -> Vec<DoctorIssue> {
        let store = self.config.store_root_path();
        let mut issues = Vec::new();

        for rec in &report.records {
            if rec.link.is_broken() {
                // 白名单技能的断链仍然是真故障，照样报；但要说明「修复计划会跳过它」，
                // 否则用户点了修复只拿到一句「所有链接都健康」，看起来像工具失灵。
                let mut message = t!(Msg::DoctorBrokenLink, target = rec.link.label());
                if rec.agent_local {
                    message.push_str(&t!(Msg::DoctorAgentLocalSuffix));
                }
                issues.push(DoctorIssue {
                    level: "danger".into(),
                    path: rec.path.clone(),
                    message,
                    kind: "broken_link".into(),
                    subject: Some(rec.name.clone()),
                });
            }
            if matches!(rec.link, LinkState::Foreign { .. }) {
                issues.push(DoctorIssue {
                    level: "warn".into(),
                    path: rec.path.clone(),
                    message: t!(Msg::DoctorForeignLink),
                    kind: "foreign_link".into(),
                    subject: Some(rec.name.clone()),
                });
            }
            if rec.source == crate::model::skill::SkillSource::AgentCopy
                && matches!(rec.link, LinkState::RealDir)
                && report.duplicates.iter().any(|d| d.name == rec.name)
            {
                issues.push(DoctorIssue {
                    level: "warn".into(),
                    path: rec.path.clone(),
                    message: t!(Msg::DoctorDuplicateCopy),
                    kind: "duplicate_copy".into(),
                    subject: Some(rec.name.clone()),
                });
            }
        }

        for dup in &report.duplicates {
            issues.push(DoctorIssue {
                level: "info".into(),
                path: dup.suggested_source.clone(),
                message: t!(
                    Msg::DoctorDuplicateGroup,
                    name = dup.name,
                    copies = dup.members.len(),
                    size = human_bytes(dup.wasted_bytes)
                ),
                kind: "duplicate_group".into(),
                subject: Some(dup.name.clone()),
            });
        }

        if !store.exists() {
            issues.push(DoctorIssue {
                level: "danger".into(),
                path: store,
                message: t!(Msg::DoctorStoreMissing),
                kind: "store_missing".into(),
                subject: None,
            });
        }

        issues.sort_by(|a, b| a.level.cmp(&b.level));
        issues
    }

    /// 已被多个 agent 共享的技能（用于总览统计）。
    pub fn shared_skills(&self, report: &ScanReport) -> Vec<SkillRecord> {
        let mut out = Vec::new();
        for rec in &report.records {
            if rec.agent.is_some() && rec.link.is_linked() {
                out.push(rec.clone());
            }
        }
        out
    }
}

pub fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut idx = 0;
    while value >= 1024.0 && idx < UNITS.len() - 1 {
        value /= 1024.0;
        idx += 1;
    }
    if idx == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[idx])
    }
}
