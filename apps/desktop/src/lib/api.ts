import { demo } from "./demo";
import type {
  ApplyReport,
  ApplyResult,
  BindProviderInput,
  CatalogEntry,
  ChangeSet,
  DeleteOptions,
  FeatureFlags,
  FeatureKey,
  GatewayConfigInput,
  GatewayRequestLog,
  GatewayRouteView,
  GatewayStatusView,
  GatewayView,
  InstructionOverview,
  LinkStrategyName,
  LocaleModeKey,
  LocaleView,
  MarketAgent,
  MarketInstallInput,
  MarketPage,
  MarketSkill,
  MarketSkillDetail,
  MarketSort,
  McpServerView,
  ModelPrice,
  ModelPriceInput,
  Overview,
  PricingView,
  ProjectSummary,
  ProviderInput,
  ModelSlots,
  PoolMode,
  ProbeInput,
  ProbeResult,
  ProvidersView,
  RouteDeleteResult,
  SupplierDef,
  SettingsInput,
  SettingsView,
  StrategyConfig,
  UsageRange,
  UsageReport,
} from "./types";

interface TauriInternals {
  invoke: <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;
}

const internals = (
  window as unknown as { __TAURI_INTERNALS__?: TauriInternals }
).__TAURI_INTERNALS__;

export const isTauri = Boolean(internals);

/** 底层 IPC。终端这类「纯壳层能力」直接用它，不塞进下面的 api 门面。 */
export function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!internals) {
    return Promise.reject(new Error(`演示模式没有实现命令 ${cmd}`));
  }
  return internals.invoke<T>(cmd, args);
}

/** 演示模式（浏览器里直接打开）用内置样本，真实桌面端走 Tauri IPC。 */
export const api = {
  overview(): Promise<Overview> {
    return internals ? invoke<Overview>("hub_overview") : Promise.resolve(demo.overview());
  },
  instructions(): Promise<InstructionOverview> {
    return internals
      ? invoke<InstructionOverview>("hub_instructions")
      : Promise.resolve(demo.instructions());
  },
  projects(): Promise<ProjectSummary[]> {
    return internals
      ? invoke<ProjectSummary[]>("hub_projects")
      : Promise.resolve(demo.projects());
  },
  /**
   * `baseline` 指定「以哪一份副本为准」；null 时后端按体积最大者挑。
   * 传了但不在该技能组里、或不是实体目录时，后端会回退并在 note 里说明。
   */
  planDedupe(only?: string[], baseline?: string | null): Promise<ChangeSet> {
    return internals
      ? invoke<ChangeSet>("plan_dedupe", {
          only: only && only.length ? only : null,
          baseline: baseline ?? null,
        })
      : Promise.resolve(demo.planDedupe(baseline));
  },
  planRepair(): Promise<ChangeSet> {
    return internals
      ? invoke<ChangeSet>("plan_repair")
      : Promise.resolve(demo.planRepair());
  },
  /** `project = null` 表示全局作用域；项目作用域的计划必须用同一 project 执行。 */
  apply(plan: ChangeSet, project?: string | null): Promise<ApplyResult> {
    return internals
      ? invoke<ApplyResult>("apply_plan", { plan, project: project ?? null })
      : Promise.resolve(demo.apply());
  },
  mcpView(project?: string | null): Promise<McpServerView[]> {
    return internals
      ? invoke<McpServerView[]>("mcp_view", { project: project ?? null })
      : Promise.resolve(demo.mcpView(project ?? null));
  },
  planMcpWrite(
    project: string | null,
    servers: McpServerView[],
    agents: string[]
  ): Promise<ChangeSet> {
    return internals
      ? invoke<ChangeSet>("plan_mcp_write", { project, servers, agents })
      : Promise.resolve(demo.planMcpWrite(project, servers, agents));
  },
  rollback(batch: string): Promise<string[]> {
    return internals
      ? invoke<string[]>("rollback", { batch })
      : Promise.resolve(demo.rollback(batch));
  },
  /**
   * 常见 Agent 目录。`probeVersions` 为 true 时会启动子进程读版本，
   * 只应由「检测版本」按钮触发。
   */
  agentCatalog(probeVersions = false): Promise<CatalogEntry[]> {
    return internals
      ? invoke<CatalogEntry[]>("hub_agent_catalog", { probeVersions })
      : Promise.resolve(demo.agentCatalog(probeVersions));
  },
  /** 登记为受管平台：后端只写 config.json，不创建目录、不动已有链接。 */
  addManagedAgent(key: string): Promise<CatalogEntry[]> {
    return internals
      ? invoke<CatalogEntry[]>("add_managed_agent", { key })
      : Promise.resolve(demo.addManagedAgent(key));
  },
  removeManagedAgent(key: string): Promise<CatalogEntry[]> {
    return internals
      ? invoke<CatalogEntry[]>("remove_managed_agent", { key })
      : Promise.resolve(demo.removeManagedAgent(key));
  },
  /** 设置页读取：路径生效值、仓库、策略、自定义平台。 */
  settings(): Promise<SettingsView> {
    return internals
      ? invoke<SettingsView>("hub_settings")
      : Promise.resolve(demo.settings());
  },
  /** 全量保存；后端校验失败会 reject，文案可直接展示。 */
  saveSettings(input: SettingsInput): Promise<SettingsView> {
    return internals
      ? invoke<SettingsView>("save_settings", { input })
      : Promise.resolve(demo.saveSettings(input));
  },
  /** Phase 3：provider 注册表 + 各 agent 绑定状态（密钥已脱敏）。 */
  providers(): Promise<ProvidersView> {
    return internals
      ? invoke<ProvidersView>("hub_providers")
      : Promise.resolve(demo.providers());
  },
  /** 新增 / 编辑 provider：只写 config.json，不碰 agent 配置。 */
  saveProvider(input: ProviderInput): Promise<ProvidersView> {
    return internals
      ? invoke<ProvidersView>("save_provider", { input })
      : Promise.resolve(demo.saveProvider(input));
  },
  deleteProvider(id: string): Promise<ProvidersView> {
    return internals
      ? invoke<ProvidersView>("delete_provider", { id })
      : Promise.resolve(demo.deleteProvider(id));
  },
  bindProvider(input: BindProviderInput): Promise<ProvidersView> {
    return internals
      ? invoke<ProvidersView>("bind_provider", { input })
      : Promise.resolve(demo.bindProvider(input));
  },
  unbindProvider(agent: string): Promise<ProvidersView> {
    return internals
      ? invoke<ProvidersView>("unbind_provider", { agent })
      : Promise.resolve(demo.unbindProvider(agent));
  },
  /** 按已保存的绑定生成写入计划（只出计划，执行仍走 apply）。 */
  planProviderApply(agent: string): Promise<ChangeSet> {
    return internals
      ? invoke<ChangeSet>("plan_provider_apply", { agent })
      : Promise.resolve(demo.planProviderApply(agent));
  },

  // ---- 供应商池（每一步都会写入并探测） ----
  addToPool(agent: string, providerId: string): Promise<ApplyReport> {
    return internals
      ? invoke<ApplyReport>("add_to_pool", { agent, providerId })
      : Promise.resolve(demo.addToPool(agent, providerId));
  },
  removeFromPool(agent: string, providerId: string): Promise<ApplyReport> {
    return internals
      ? invoke<ApplyReport>("remove_from_pool", { agent, providerId })
      : Promise.resolve(demo.removeFromPool(agent, providerId));
  },
  setDefaultProvider(agent: string, providerId: string): Promise<ApplyReport> {
    return internals
      ? invoke<ApplyReport>("set_default_provider", { agent, providerId })
      : Promise.resolve(demo.setDefaultProvider(agent, providerId));
  },
  setPoolMode(agent: string, mode: PoolMode): Promise<ApplyReport> {
    return internals
      ? invoke<ApplyReport>("set_pool_mode", { agent, mode })
      : Promise.resolve(demo.setPoolMode(agent, mode));
  },
  /** 只更新 Claude 的四个模型槽位，不碰默认供应商与模型 */
  setModelSlots(agent: string, slots: ModelSlots): Promise<ApplyReport> {
    return internals
      ? invoke<ApplyReport>("set_model_slots", { agent, slots })
      : Promise.resolve(demo.setModelSlots(agent, slots));
  },

  /** 一键应用：写入 + 探测一次。切换器点 chip 走这条。 */
  applyProviderBinding(agent: string): Promise<ApplyReport> {
    return internals
      ? invoke<ApplyReport>("apply_provider_binding", { agent })
      : Promise.resolve(demo.applyProviderBinding(agent));
  },

  /** 内置供应商目录（静态表，不联网）。 */
  suppliers(keyword = ""): Promise<SupplierDef[]> {
    return internals
      ? invoke<SupplierDef[]>("hub_suppliers", { keyword })
      : Promise.resolve(demo.suppliers(keyword));
  },
  /** 从供应商的 /models 拉模型列表；只有用户点按钮时才会联网。 */
  probeProviderModels(input: ProbeInput): Promise<ProbeResult> {
    return internals
      ? invoke<ProbeResult>("probe_provider_models", { input })
      : Promise.resolve(demo.probeProviderModels(input));
  },

  /**
   * 功能开关当前状态。独立命令而不是从 settings 里取：
   * 导航过滤要在启动时就知道结果，不该为此付一次仓库目录统计。
   */
  features(): Promise<FeatureFlags> {
    return internals
      ? invoke<FeatureFlags>("hub_features")
      : Promise.resolve(demo.features());
  },
  /** 翻转一个功能开关；即时写 config.json，不进 ChangeSet。 */
  setFeature(key: FeatureKey, enabled: boolean): Promise<FeatureFlags> {
    return internals
      ? invoke<FeatureFlags>("set_feature", { key, enabled })
      : Promise.resolve(demo.setFeature(key, enabled));
  },

  // ---- 界面语言 ----
  /** 读权威语言选择（`config.json`），启动时与设置页回显用 */
  locale(): Promise<LocaleView> {
    return internals ? invoke<LocaleView>("hub_locale") : Promise.resolve(demo.locale());
  },
  /**
   * 切换语言并立即落盘。
   *
   * `effective` 是前端已经解析好的实际语言：后端读环境变量拿不到可靠系统语言，
   * 而 webview 的 `navigator.languages` 就是系统设置本身。
   */
  setLocale(mode: LocaleModeKey, effective: string): Promise<LocaleView> {
    return internals
      ? invoke<LocaleView>("hub_set_locale", { mode, effective })
      : Promise.resolve(demo.setLocale(mode, effective));
  },

  // ---- Provider 网关 ----
  gateway(): Promise<GatewayView> {
    return internals ? invoke<GatewayView>("hub_gateway") : Promise.resolve(demo.gateway());
  },
  saveGatewayConfig(input: GatewayConfigInput): Promise<GatewayView> {
    return internals
      ? invoke<GatewayView>("save_gateway_config", { input })
      : Promise.resolve(demo.saveGatewayConfig(input));
  },
  saveGatewayRoute(route: GatewayRouteView): Promise<GatewayView> {
    return internals
      ? invoke<GatewayView>("save_gateway_route", { route })
      : Promise.resolve(demo.saveGatewayRoute(route));
  },
  deleteGatewayRoute(id: string): Promise<RouteDeleteResult> {
    return internals
      ? invoke<RouteDeleteResult>("delete_gateway_route", { id })
      : Promise.resolve(demo.deleteGatewayRoute(id));
  },
  gatewayStatus(): Promise<GatewayStatusView> {
    return internals
      ? invoke<GatewayStatusView>("gateway_status")
      : Promise.resolve(demo.gatewayStatus());
  },
  gatewayStart(): Promise<GatewayStatusView> {
    return internals
      ? invoke<GatewayStatusView>("gateway_start")
      : Promise.resolve(demo.gatewayStart());
  },
  gatewayStop(): Promise<GatewayStatusView> {
    return internals
      ? invoke<GatewayStatusView>("gateway_stop")
      : Promise.resolve(demo.gatewayStop());
  },
  gatewayRestart(): Promise<GatewayStatusView> {
    return internals
      ? invoke<GatewayStatusView>("gateway_restart")
      : Promise.resolve(demo.gatewayRestart());
  },
  gatewayLogs(limit = 200): Promise<GatewayRequestLog[]> {
    return internals
      ? invoke<GatewayRequestLog[]>("gateway_logs", { limit })
      : Promise.resolve(demo.gatewayLogs(limit));
  },
  clearGatewayLogs(): Promise<void> {
    return internals ? invoke<void>("clear_gateway_logs") : Promise.resolve(demo.clearGatewayLogs());
  },
  /** 把经网关的 agent 退回直连的写入计划 */
  planRevertToDirect(agents: string[]): Promise<ChangeSet> {
    return internals
      ? invoke<ChangeSet>("plan_revert_to_direct", { agents })
      : Promise.resolve(demo.planRevertToDirect(agents));
  },

  /** `strategy = null` 表示清除覆盖，回到「跟随平台默认」。 */
  setSkillStrategy(
    name: string,
    strategy: LinkStrategyName | null
  ): Promise<StrategyConfig> {
    return internals
      ? invoke<StrategyConfig>("set_skill_strategy", { name, strategy })
      : Promise.resolve(demo.setSkillStrategy(name, strategy));
  },

  /**
   * 标记 / 取消「agent 独有技能」。只写 config.json，不动磁盘、不拆已存在的链接。
   * 调用方拿到结果后应重扫 —— 标记来自扫描结果，只改本地 state 会不一致。
   */
  setAgentLocalSkill(name: string, on: boolean): Promise<string[]> {
    return internals
      ? invoke<string[]>("set_agent_local_skill", { name, on })
      : Promise.resolve(demo.setAgentLocalSkill(name, on));
  },

  /** 删除技能计划：只传路径，后端重扫定位（只读 / 未知路径会被跳过）。 */
  planDelete(paths: string[], opts: DeleteOptions): Promise<ChangeSet> {
    return internals
      ? invoke<ChangeSet>("plan_delete", {
          paths,
          useTrash: opts.use_trash,
          withLinks: opts.with_links,
        })
      : Promise.resolve(demo.planDelete(paths, opts));
  },

  /** 本机可安装的智能体（只有 available 的能勾选）。 */
  marketAgents(): Promise<MarketAgent[]> {
    return internals
      ? invoke<MarketAgent[]>("market_agents")
      : Promise.resolve(demo.marketAgents());
  },

  /** 搜索 / 浏览一页：关键词为空时按 sort 浏览；cursor = 上一页返回的游标。 */
  marketSearch(
    query: string,
    sort: MarketSort,
    cursor: string | null = null,
    limit = 50
  ): Promise<MarketPage> {
    return internals
      ? invoke<MarketPage>("market_search", { query, sort, cursor, limit })
      : Promise.resolve(demo.marketSearch(query, sort, cursor, limit));
  },

  marketDetail(slug: string): Promise<MarketSkillDetail> {
    return internals
      ? invoke<MarketSkillDetail>("market_detail", { slug })
      : Promise.resolve(demo.marketDetail(slug));
  },

  /** 生成安装计划（内含下载），落盘仍走 apply。 */
  planMarketInstall(input: MarketInstallInput): Promise<ChangeSet> {
    return internals
      ? invoke<ChangeSet>("plan_market_install", { input })
      : Promise.resolve(demo.planMarketInstall(input));
  },

  /* ------------------------------------------------------------ Phase 4 用量 */

  /**
   * Token 用量。首次进入可能要解析几百 MB 日志（后端在独立线程里跑），
   * 之后靠按文件缓存只读元数据，所以不要把这里放进全局 refresh。
   */
  usage(range: UsageRange, refresh = false): Promise<UsageReport> {
    return internals
      ? invoke<UsageReport>("hub_usage", { range, refresh })
      : Promise.resolve(demo.usage(range));
  },

  usagePricing(): Promise<PricingView> {
    return internals
      ? invoke<PricingView>("hub_usage_pricing")
      : Promise.resolve(demo.usagePricing());
  },

  saveUsagePricing(rows: ModelPriceInput[]): Promise<PricingView> {
    return internals
      ? invoke<PricingView>("save_usage_pricing", { rows })
      : Promise.resolve(demo.saveUsagePricing(rows));
  },

  /** 只会联网拉取并解析，不写盘；用户确认后再调 `applyUsagePricing`。 */
  previewUsagePricing(source: string): Promise<ModelPrice[]> {
    return internals
      ? invoke<ModelPrice[]>("preview_usage_pricing", { source })
      : Promise.resolve(demo.previewUsagePricing(source));
  },

  applyUsagePricing(rows: ModelPrice[]): Promise<PricingView> {
    return internals
      ? invoke<PricingView>("apply_usage_pricing", { rows })
      : Promise.resolve(demo.applyUsagePricing(rows));
  },
};
