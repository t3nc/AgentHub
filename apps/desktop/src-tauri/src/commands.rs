use std::collections::BTreeMap;
use std::path::PathBuf;

use agenthub_core::apply::DoctorIssue;
use agenthub_core::ext::catalog::CatalogEntry;
use agenthub_core::link::LinkStrategy;
use agenthub_core::model::agent::{AgentId, Scope};
use agenthub_core::model::change::DeleteOptions;
use agenthub_core::model::change::{ApplyResult, BatchRecord, ChangeSet};
use agenthub_core::model::gateway::{FeatureFlags, FeatureKey};
use agenthub_core::model::instruction::{ChainEntry, InstructionSpec};
use agenthub_core::model::market::{
    MarketAgent, MarketInstallInput, MarketPage, MarketSkillDetail, MarketSort,
};
use agenthub_core::model::mcp::{McpServer, McpServerView};
use agenthub_core::project::{self, ProjectSummary};
use agenthub_core::scan::ScanReport;
use agenthub_core::{AgentHub, Error, paths};
use serde::Serialize;
use tauri::{Manager, State};

use crate::AppState;
use crate::gateway::{GatewayState, GatewayStatusView, ReloadOutcome};

fn flat(err: Error) -> String {
    err.to_string()
}

/// 前端统一传 `project: Option<String>`，这里解析成 Scope（与 CLI 的 `scope_of` 同义）。
///
/// `None` / 空串 = 全局作用域；给了项目就必须是存在且为绝对路径的目录，
/// 否则后面 `ensure_within` 会因为「路径不在受管根内」给出难以理解的错误。
fn scope_of(project: Option<String>) -> Result<Scope, String> {
    let Some(raw) = project else {
        return Ok(Scope::User);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Scope::User);
    }
    let root = PathBuf::from(trimmed);
    project::require_root(&root).map_err(flat)?;
    if !root.is_dir() {
        return Err(format!("项目目录不存在或不是目录：{}", root.display()));
    }
    Ok(Scope::Project {
        root: paths::normalize_lexical(&root),
    })
}

/// 链接策略的当前解析结果，供 UI 回显「跟随平台默认 → 目录联接」这类文案。
#[derive(Serialize)]
pub struct StrategyConfigDto {
    /// 全局默认解析后的值（配置默认，或平台默认）
    pub default: LinkStrategy,
    /// 平台默认（mac/Linux 软链，Windows 目录联接）
    pub platform: LinkStrategy,
    /// 单条技能覆盖
    pub overrides: BTreeMap<String, LinkStrategy>,
}

fn strategy_config(hub: &AgentHub) -> StrategyConfigDto {
    StrategyConfigDto {
        default: hub.config.strategy_default(),
        platform: LinkStrategy::platform_default(),
        overrides: hub.config.skill_strategy_overrides.clone(),
    }
}

#[derive(Serialize)]
pub struct OverviewDto {
    pub report: ScanReport,
    pub mcp: Vec<McpServerView>,
    pub batches: Vec<BatchRecord>,
    pub doctor: Vec<DoctorIssue>,
    pub data_root: String,
    pub platform: String,
    pub strategies: StrategyConfigDto,
    pub demo: bool,
}

#[derive(Serialize)]
pub struct InstructionDto {
    pub specs: Vec<InstructionSpec>,
    pub chain: Vec<ChainEntry>,
}

#[tauri::command]
pub fn hub_overview(state: State<'_, AppState>) -> Result<OverviewDto, String> {
    let hub = state.0.lock().map_err(|e| e.to_string())?;
    let report = hub.scan(&Scope::User).map_err(flat)?;
    let mcp = hub.mcp_overview(&Scope::User).map_err(flat)?;
    let batches = hub.batches(12).map_err(flat)?;
    // 复用上面那份扫描结果，不再整盘扫第二遍
    let doctor = hub.doctor_from(&report);
    Ok(OverviewDto {
        report,
        mcp,
        batches,
        doctor,
        data_root: paths::path_str(&paths::data_root()),
        platform: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
        strategies: strategy_config(&hub),
        demo: false,
    })
}

#[tauri::command]
pub fn hub_instructions(state: State<'_, AppState>) -> Result<InstructionDto, String> {
    let hub = state.0.lock().map_err(|e| e.to_string())?;
    let (specs, chain) = hub.instructions_overview(&Scope::User);
    Ok(InstructionDto { specs, chain })
}

#[tauri::command]
pub fn hub_projects(state: State<'_, AppState>) -> Result<Vec<ProjectSummary>, String> {
    let hub = state.0.lock().map_err(|e| e.to_string())?;
    Ok(hub.projects())
}

#[tauri::command]
pub fn plan_dedupe(
    state: State<'_, AppState>,
    only: Option<Vec<String>>,
    // 指定以哪一份副本为准；None 时按体积最大的实体副本
    baseline: Option<PathBuf>,
) -> Result<ChangeSet, String> {
    let hub = state.0.lock().map_err(|e| e.to_string())?;
    hub.plan_dedupe(only.as_deref(), baseline.as_deref())
        .map_err(flat)
}

#[tauri::command]
pub fn plan_repair(state: State<'_, AppState>) -> Result<ChangeSet, String> {
    let hub = state.0.lock().map_err(|e| e.to_string())?;
    hub.plan_repair().map_err(flat)
}

#[tauri::command]
pub fn apply_plan(
    state: State<'_, AppState>,
    plan: ChangeSet,
    project: Option<String>,
) -> Result<ApplyResult, String> {
    // 项目作用域生成的计划必须用同一作用域执行，否则项目内路径会被写入门禁拒绝
    let scope = scope_of(project)?;
    let mut hub = state.0.lock().map_err(|e| e.to_string())?;
    if let Scope::Project { root } = &scope {
        hub.remember_project(root);
    }
    hub.apply(&plan, &scope).map_err(flat)
}

/// 指定作用域的 MCP 分发视图（全局 or 某个项目）。
#[tauri::command]
pub fn mcp_view(
    state: State<'_, AppState>,
    project: Option<String>,
) -> Result<Vec<McpServerView>, String> {
    let scope = scope_of(project)?;
    let hub = state.0.lock().map_err(|e| e.to_string())?;
    hub.mcp_overview(&scope).map_err(flat)
}

/// 生成 MCP 写入计划。`project` 为空时写全局配置。
#[tauri::command]
pub fn plan_mcp_write(
    state: State<'_, AppState>,
    project: Option<String>,
    servers: Vec<McpServer>,
    agents: Vec<String>,
) -> Result<ChangeSet, String> {
    let scope = scope_of(project)?;
    let hub = state.0.lock().map_err(|e| e.to_string())?;
    let ids: Vec<AgentId> = agents
        .iter()
        .map(|a| AgentId::from_slug(a.trim()))
        .collect();
    hub.plan_mcp_write(&servers, &ids, &scope).map_err(flat)
}

#[tauri::command]
pub fn rollback(state: State<'_, AppState>, batch: String) -> Result<Vec<String>, String> {
    let hub = state.0.lock().map_err(|e| e.to_string())?;
    hub.rollback(&batch).map_err(flat)
}

/// 常见 Agent 目录：检测 + 安装指引。`probe_versions` 为 true 时会起子进程读版本，
/// 只能由用户显式点击触发。
#[tauri::command]
pub fn hub_agent_catalog(
    state: State<'_, AppState>,
    probe_versions: bool,
) -> Result<Vec<CatalogEntry>, String> {
    let hub = state.0.lock().map_err(|e| e.to_string())?;
    Ok(hub.agent_catalog(probe_versions))
}

/// 把目录里的产品登记为受管平台：只写 config.json，不创建目录、不动已有链接。
#[tauri::command]
pub fn add_managed_agent(
    state: State<'_, AppState>,
    key: String,
) -> Result<Vec<CatalogEntry>, String> {
    let mut hub = state.0.lock().map_err(|e| e.to_string())?;
    hub.add_managed_agent(&key).map_err(flat)
}

/// 移出受管：只删配置里的登记，磁盘上的目录与链接保持原样。
#[tauri::command]
pub fn remove_managed_agent(
    state: State<'_, AppState>,
    key: String,
) -> Result<Vec<CatalogEntry>, String> {
    let mut hub = state.0.lock().map_err(|e| e.to_string())?;
    hub.remove_managed_agent(&key).map_err(flat)
}

/// 设置页读取：每个 agent 的生效路径 + 仓库 / 策略 / 自定义平台。
#[tauri::command]
pub fn hub_settings(
    state: State<'_, AppState>,
) -> Result<agenthub_core::api::settings::SettingsView, String> {
    let hub = state.0.lock().map_err(|e| e.to_string())?;
    Ok(hub.settings_view())
}

/// 设置页保存：全量提交，后端校验后写 config.json 并重建适配器。
#[tauri::command]
pub fn save_settings(
    state: State<'_, AppState>,
    input: agenthub_core::api::settings::SettingsInput,
) -> Result<agenthub_core::api::settings::SettingsView, String> {
    let mut hub = state.0.lock().map_err(|e| e.to_string())?;
    hub.save_settings(input).map_err(flat)
}

/// Phase 3：provider 注册表与各 agent 的绑定状态（密钥已脱敏）。
#[tauri::command]
pub fn hub_providers(
    state: State<'_, AppState>,
) -> Result<agenthub_core::api::provider::ProvidersView, String> {
    let hub = state.0.lock().map_err(|e| e.to_string())?;
    Ok(hub.providers_view())
}

/// Phase 4：token 用量报告。
///
/// 必须是 `async` + `spawn_blocking`：首次进入要读几百 MB 会话日志，同步命令跑在
/// Tauri 主线程上会把窗口冻住；同时不能在 `await` 期间持有 `Mutex<AgentHub>`。
#[tauri::command]
pub async fn hub_usage(
    range: String,
    refresh: Option<bool>,
) -> Result<agenthub_core::api::usage::UsageReport, String> {
    let range = agenthub_core::api::usage::UsageRange::parse(&range);
    tauri::async_runtime::spawn_blocking(move || {
        let hub = AgentHub::bootstrap().map_err(flat)?;
        hub.usage_report(range, refresh.unwrap_or(false))
            .map_err(flat)
    })
    .await
    .map_err(|err| format!("统计 Token 用量失败: {err}"))?
}

/// 价格表读取只碰 `config.json`，不必为它打开索引库。
#[tauri::command]
pub fn hub_usage_pricing() -> Result<agenthub_core::api::pricing::PricingView, String> {
    let config = agenthub_core::AppConfig::load();
    Ok(agenthub_core::api::pricing::view(&config.usage_pricing))
}

#[tauri::command]
pub fn save_usage_pricing(
    state: State<'_, AppState>,
    rows: Vec<agenthub_core::api::pricing::ModelPriceInput>,
) -> Result<agenthub_core::api::pricing::PricingView, String> {
    let mut hub = state.0.lock().map_err(|e| e.to_string())?;
    hub.save_usage_pricing(&rows).map_err(flat)
}

/// 导入预览：联网拉取但**不写盘**，返回候选行给前端勾选。
#[tauri::command]
pub async fn preview_usage_pricing(
    source: String,
) -> Result<Vec<agenthub_core::ext::pricing::ModelPrice>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let hub = AgentHub::bootstrap().map_err(flat)?;
        hub.preview_usage_pricing(&source).map_err(flat)
    })
    .await
    .map_err(|err| format!("导入价格失败: {err}"))?
}

#[tauri::command]
pub fn apply_usage_pricing(
    state: State<'_, AppState>,
    rows: Vec<agenthub_core::ext::pricing::ModelPrice>,
) -> Result<agenthub_core::api::pricing::PricingView, String> {
    let mut hub = state.0.lock().map_err(|e| e.to_string())?;
    hub.apply_usage_pricing(&rows).map_err(flat)
}

/// 新增 / 编辑 provider：只写 `config.json`，不碰任何 agent 配置。
#[tauri::command]
pub fn save_provider(
    state: State<'_, AppState>,
    input: agenthub_core::api::provider::ProviderInput,
) -> Result<agenthub_core::api::provider::ProvidersView, String> {
    let mut hub = state.0.lock().map_err(|e| e.to_string())?;
    hub.upsert_provider(input).map_err(flat)
}

#[tauri::command]
pub fn delete_provider(
    state: State<'_, AppState>,
    id: String,
) -> Result<agenthub_core::api::provider::ProvidersView, String> {
    let mut hub = state.0.lock().map_err(|e| e.to_string())?;
    hub.delete_provider(&id).map_err(flat)
}

/// 把 provider 绑定到某个 agent（可指定模型与「设为默认」）。
#[tauri::command]
pub fn bind_provider(
    state: State<'_, AppState>,
    input: agenthub_core::api::provider::BindInput,
) -> Result<agenthub_core::api::provider::ProvidersView, String> {
    let mut hub = state.0.lock().map_err(|e| e.to_string())?;
    hub.bind_provider(input).map_err(flat)
}

#[tauri::command]
pub fn unbind_provider(
    state: State<'_, AppState>,
    agent: String,
) -> Result<agenthub_core::api::provider::ProvidersView, String> {
    let mut hub = state.0.lock().map_err(|e| e.to_string())?;
    hub.unbind_provider(&agent).map_err(flat)
}

/// 按已保存的绑定生成写入计划（只出计划，落盘仍走 `apply_plan`）。
#[tauri::command]
pub fn plan_provider_apply(state: State<'_, AppState>, agent: String) -> Result<ChangeSet, String> {
    let hub = state.0.lock().map_err(|e| e.to_string())?;
    hub.plan_provider_apply(&agent).map_err(flat)
}

/// 设置或清除单条技能的链接策略；`strategy = null` 表示回到「跟随平台默认」。
/// 返回更新后的策略解析结果，前端直接拿它回显，不需要再扫一遍。
#[tauri::command]
pub fn set_skill_strategy(
    state: State<'_, AppState>,
    name: String,
    strategy: Option<LinkStrategy>,
) -> Result<StrategyConfigDto, String> {
    let mut hub = state.0.lock().map_err(|e| e.to_string())?;
    hub.set_skill_strategy(&name, strategy).map_err(flat)?;
    Ok(strategy_config(&hub))
}

/// 标记 / 取消「agent 独有技能」（computer-use 这类各家实现互不通用的技能）。
///
/// 只写 `config.json`，不动磁盘上的技能目录，也不拆已存在的链接。
/// 返回归一化后的完整名单；前端拿到后要重扫一次（标记来自扫描结果）。
#[tauri::command]
pub fn set_agent_local_skill(
    state: State<'_, AppState>,
    name: String,
    on: bool,
) -> Result<Vec<String>, String> {
    let mut hub = state.0.lock().map_err(|e| e.to_string())?;
    hub.set_agent_local_skill(&name, on).map_err(flat)
}

/// 删除技能（多选）。前端只传路径，后端重新扫描定位，只读项与未知路径都会被跳过。
#[tauri::command]
pub fn plan_delete(
    state: State<'_, AppState>,
    paths: Vec<PathBuf>,
    use_trash: Option<bool>,
    with_links: Option<bool>,
) -> Result<ChangeSet, String> {
    let hub = state.0.lock().map_err(|e| e.to_string())?;
    let opts = DeleteOptions {
        use_trash: use_trash.unwrap_or(true),
        with_links: with_links.unwrap_or(true),
    };
    hub.plan_delete(&paths, opts).map_err(flat)
}

/// 本机可安装的智能体（只有「检测到 + 有可写技能目录」才是 available）。
#[tauri::command]
pub fn market_agents(state: State<'_, AppState>) -> Result<Vec<MarketAgent>, String> {
    let hub = state.0.lock().map_err(|e| e.to_string())?;
    Ok(hub.market_agents())
}

/// 市场搜索 / 浏览一页。走 `spawn_blocking`：HTTP 最长 20s，绝不能占住主线程。
///
/// `cursor` 对这里是黑盒：前端把上一页返回的值原样传回来即可。
#[tauri::command]
pub async fn market_search(
    app: tauri::AppHandle,
    query: String,
    sort: Option<MarketSort>,
    cursor: Option<String>,
    limit: Option<usize>,
) -> Result<MarketPage, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let hub = state.0.lock().map_err(|e| e.to_string())?;
        hub.market_search(
            &query,
            sort.unwrap_or(MarketSort::Recommended),
            cursor.as_deref(),
            limit.unwrap_or(agenthub_core::market::PAGE_SIZE_DEFAULT),
        )
        .map_err(flat)
    })
    .await
    .map_err(|e| format!("市场请求失败：{e}"))?
}

/// 技能详情（含正文、文件清单与安全审核结论）。
#[tauri::command]
pub async fn market_detail(
    app: tauri::AppHandle,
    slug: String,
) -> Result<MarketSkillDetail, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let hub = state.0.lock().map_err(|e| e.to_string())?;
        hub.market_detail(&slug).map_err(flat)
    })
    .await
    .map_err(|e| format!("市场请求失败：{e}"))?
}

/// 生成安装计划：下载到缓存 → 搬进中央仓库 → 给所选 agent 建链接。落盘仍走 `apply_plan`。
#[tauri::command]
pub async fn plan_market_install(
    app: tauri::AppHandle,
    input: MarketInstallInput,
) -> Result<ChangeSet, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let hub = state.0.lock().map_err(|e| e.to_string())?;
        hub.plan_market_install(&input).map_err(flat)
    })
    .await
    .map_err(|e| format!("安装技能失败：{e}"))?
}

// ---- 功能管理（设置 › 功能管理） ----

/// 开关状态。独立命令而不是塞进 `hub_settings`：导航过滤在首帧就要结果，
/// 不该为此付一次「仓库目录占用统计」的代价。
#[tauri::command]
pub fn hub_features(state: State<'_, AppState>) -> Result<FeatureFlags, String> {
    let hub = state.0.lock().map_err(|e| e.to_string())?;
    Ok(hub.config.features.clone())
}

/// 翻转一个功能开关。即时写 `config.json`，不进 Plan/Apply。
///
/// 关掉 Provider 时**先停网关**：否则会出现「功能已禁用、端口还在监听」的错位状态。
#[tauri::command]
pub fn set_feature(
    state: State<'_, AppState>,
    gateway: State<'_, GatewayState>,
    key: String,
    enabled: bool,
) -> Result<FeatureFlags, String> {
    let feature = FeatureKey::parse(&key).ok_or_else(|| format!("未知功能：{key}"))?;
    let mut hub = state.0.lock().map_err(|e| e.to_string())?;
    let flags = hub.set_feature(&key, enabled).map_err(flat)?;
    drop(hub);
    if !enabled && matches!(feature, FeatureKey::Provider) {
        gateway.stop();
    }
    Ok(flags)
}

/// 当前界面语言（用户选择 + 实际生效）。
#[tauri::command]
pub fn hub_locale(state: State<'_, AppState>) -> Result<agenthub_core::api::LocaleView, String> {
    let hub = state.0.lock().map_err(|e| e.to_string())?;
    Ok(hub.locale_view())
}

/// 切换界面语言：即时写 `config.json`，不进 Plan/Apply。
///
/// `effective` 由前端用 `navigator.languages` 解析后传入：后端读环境变量拿不到
/// 可靠的系统语言（Windows 尤其），而 webview 里的 `navigator` 就是系统设置本身。
#[tauri::command]
pub fn hub_set_locale(
    state: State<'_, AppState>,
    mode: String,
    effective: Option<String>,
) -> Result<agenthub_core::api::LocaleView, String> {
    let mut hub = state.0.lock().map_err(|e| e.to_string())?;
    hub.set_locale(&mode, effective.as_deref()).map_err(flat)
}

/// 一键应用：把该 agent 的当前绑定写进配置文件，并立即探测一次连通性。
///
/// 切换器点 chip 走的就是这条链。写入与探测都在 `spawn_blocking` 里同步完成
/// （探测最长 8 秒，期间会占住 `Mutex<AgentHub>`，这是「点一下等结果」的预期行为）。
#[tauri::command]
pub async fn apply_provider_binding(
    app: tauri::AppHandle,
    agent: String,
) -> Result<agenthub_core::api::provider::ApplyReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let hub = state.0.lock().map_err(|e| e.to_string())?;
        hub.apply_provider_binding(&agent).map_err(flat)
    })
    .await
    .map_err(|e| format!("应用供应商失败：{e}"))?
}

// ---- 供应商池：加入 / 移除 / 设为默认 / 切换模式 ----
//
// 四条都返回 `ApplyReport`：池的改动会立刻写进 agent 配置并探测一次，
// 前端不需要再单独点「应用」。

#[tauri::command]
pub async fn add_to_pool(
    app: tauri::AppHandle,
    agent: String,
    provider_id: String,
) -> Result<agenthub_core::api::provider::ApplyReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let mut hub = state.0.lock().map_err(|e| e.to_string())?;
        hub.add_to_pool(&agent, &provider_id).map_err(flat)
    })
    .await
    .map_err(|e| format!("加入供应商失败：{e}"))?
}

#[tauri::command]
pub async fn remove_from_pool(
    app: tauri::AppHandle,
    agent: String,
    provider_id: String,
) -> Result<agenthub_core::api::provider::ApplyReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let mut hub = state.0.lock().map_err(|e| e.to_string())?;
        hub.remove_from_pool(&agent, &provider_id).map_err(flat)
    })
    .await
    .map_err(|e| format!("移除供应商失败：{e}"))?
}

#[tauri::command]
pub async fn set_default_provider(
    app: tauri::AppHandle,
    agent: String,
    provider_id: String,
) -> Result<agenthub_core::api::provider::ApplyReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let mut hub = state.0.lock().map_err(|e| e.to_string())?;
        hub.set_default_provider(&agent, &provider_id).map_err(flat)
    })
    .await
    .map_err(|e| format!("设置默认供应商失败：{e}"))?
}

/// 只更新 Claude 的四个模型槽位；不碰默认供应商与模型。
#[tauri::command]
pub async fn set_model_slots(
    app: tauri::AppHandle,
    agent: String,
    slots: std::collections::BTreeMap<String, String>,
) -> Result<agenthub_core::api::provider::ApplyReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let mut hub = state.0.lock().map_err(|e| e.to_string())?;
        hub.set_model_slots(&agent, slots).map_err(flat)
    })
    .await
    .map_err(|e| format!("保存模型映射失败：{e}"))?
}

#[tauri::command]
pub async fn set_pool_mode(
    app: tauri::AppHandle,
    agent: String,
    mode: String,
) -> Result<agenthub_core::api::provider::ApplyReport, String> {
    let parsed = agenthub_core::model::gateway::PoolMode::parse(&mode)
        .ok_or_else(|| format!("未知模式：{mode}"))?;
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let mut hub = state.0.lock().map_err(|e| e.to_string())?;
        hub.set_pool_mode(&agent, parsed).map_err(flat)
    })
    .await
    .map_err(|e| format!("切换模式失败：{e}"))?
}

/// 内置供应商目录（纯静态表，不联网）。
///
/// 返回扁平列表并带 `category`：前端自己分组，后端不必为展示形态多定一套 DTO。
#[tauri::command]
pub fn hub_suppliers(keyword: Option<String>) -> Vec<agenthub_core::model::supplier::SupplierDef> {
    agenthub_core::ext::supplier::search(keyword.as_deref().unwrap_or(""))
        .into_iter()
        .cloned()
        .collect()
}

/// 从供应商的 `/models` 拉模型列表（只有用户点按钮时才联网）。
#[tauri::command]
pub async fn probe_provider_models(
    input: agenthub_core::api::provider::ProbeInput,
) -> Result<agenthub_core::api::provider::ProbeResult, String> {
    tauri::async_runtime::spawn_blocking(move || agenthub_core::api::provider::probe_models(&input))
        .await
        .map_err(|e| format!("拉取模型列表失败：{e}"))
}

// ---- Provider 网关 ----

#[tauri::command]
pub fn hub_gateway(
    state: State<'_, AppState>,
) -> Result<agenthub_core::api::gateway::GatewayView, String> {
    let hub = state.0.lock().map_err(|e| e.to_string())?;
    Ok(hub.gateway_view())
}

/// 保存网关全局配置。端口变了就地停服：热重载换不了监听端口。
#[tauri::command]
pub fn save_gateway_config(
    state: State<'_, AppState>,
    gateway: State<'_, GatewayState>,
    input: agenthub_core::api::gateway::GatewayConfigInput,
) -> Result<agenthub_core::api::gateway::GatewayView, String> {
    let mut hub = state.0.lock().map_err(|e| e.to_string())?;
    let view = hub.save_gateway_config(&input).map_err(flat)?;
    let config = hub.config.clone();
    drop(hub);
    if gateway.reload(&config) == ReloadOutcome::NeedsRestart {
        gateway.stop();
    }
    Ok(view)
}

#[tauri::command]
pub fn save_gateway_route(
    state: State<'_, AppState>,
    gateway: State<'_, GatewayState>,
    route: agenthub_core::model::gateway::GatewayRoute,
) -> Result<agenthub_core::api::gateway::GatewayView, String> {
    let mut hub = state.0.lock().map_err(|e| e.to_string())?;
    let view = hub.save_gateway_route(route).map_err(flat)?;
    let config = hub.config.clone();
    drop(hub);
    if gateway.reload(&config) == ReloadOutcome::NeedsRestart {
        gateway.stop();
    }
    Ok(view)
}

#[tauri::command]
pub fn delete_gateway_route(
    state: State<'_, AppState>,
    gateway: State<'_, GatewayState>,
    id: String,
) -> Result<agenthub_core::api::gateway::RouteDeleteResult, String> {
    let mut hub = state.0.lock().map_err(|e| e.to_string())?;
    let result = hub.delete_gateway_route(&id).map_err(flat)?;
    let config = hub.config.clone();
    drop(hub);
    if gateway.reload(&config) == ReloadOutcome::NeedsRestart {
        gateway.stop();
    }
    Ok(result)
}

/// 把一批 agent 从「经网关」退回直连的写入计划（只出计划，落盘走 `apply_plan`）。
#[tauri::command]
pub fn plan_revert_to_direct(
    state: State<'_, AppState>,
    agents: Vec<String>,
) -> Result<ChangeSet, String> {
    let hub = state.0.lock().map_err(|e| e.to_string())?;
    hub.plan_revert_to_direct(&agents).map_err(flat)
}

#[tauri::command]
pub fn gateway_status(
    state: State<'_, AppState>,
    gateway: State<'_, GatewayState>,
) -> Result<GatewayStatusView, String> {
    let hub = state.0.lock().map_err(|e| e.to_string())?;
    Ok(gateway.status(&hub.config))
}

/// 启动监听。必须先 clone 配置再 await —— 不能在 await 期间持有 `Mutex<AgentHub>`。
#[tauri::command]
pub async fn gateway_start(
    state: State<'_, AppState>,
    gateway: State<'_, GatewayState>,
) -> Result<GatewayStatusView, String> {
    let config = {
        let hub = state.0.lock().map_err(|e| e.to_string())?;
        if !hub.feature_enabled(FeatureKey::Provider) {
            return Err("功能未启用：Provider（设置 › 功能管理）".to_string());
        }
        hub.config.clone()
    };
    gateway.start(&config).await?;
    Ok(gateway.status(&config))
}

#[tauri::command]
pub fn gateway_stop(
    state: State<'_, AppState>,
    gateway: State<'_, GatewayState>,
) -> Result<GatewayStatusView, String> {
    gateway.stop();
    let hub = state.0.lock().map_err(|e| e.to_string())?;
    Ok(gateway.status(&hub.config))
}

#[tauri::command]
pub async fn gateway_restart(
    state: State<'_, AppState>,
    gateway: State<'_, GatewayState>,
) -> Result<GatewayStatusView, String> {
    let config = {
        let hub = state.0.lock().map_err(|e| e.to_string())?;
        if !hub.feature_enabled(FeatureKey::Provider) {
            return Err("功能未启用：Provider（设置 › 功能管理）".to_string());
        }
        hub.config.clone()
    };
    gateway.stop();
    gateway.start(&config).await?;
    Ok(gateway.status(&config))
}

/// 最近的请求元数据（内存环形缓冲，正文一律不记）。
#[tauri::command]
pub fn gateway_logs(
    gateway: State<'_, GatewayState>,
    limit: Option<usize>,
) -> Vec<agenthub_gateway::RequestLog> {
    gateway.logs(limit.unwrap_or(200))
}

#[tauri::command]
pub fn clear_gateway_logs(gateway: State<'_, GatewayState>) {
    gateway.clear_logs();
}
