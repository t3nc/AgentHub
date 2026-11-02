import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { listen } from "@tauri-apps/api/event";
import { api, isTauri } from "./api";
import { mergeDelta } from "./fs-change";
import { strategyLabel } from "./format";
import { tr } from "./i18n/runtime";
import type { MessageKey } from "./i18n/messages";
import {
  DEFAULT_FEATURES,
  type BindProviderInput,
  type CatalogEntry,
  type ChangeSet,
  type DeleteOptions,
  type FeatureFlags,
  type FeatureKey,
  type GatewayConfigInput,
  type GatewayRequestLog,
  type GatewayRouteView,
  type GatewayStatusView,
  type GatewayView,
  type InstructionOverview,
  type LinkStrategyName,
  type MarketAgent,
  type MarketInstallInput,
  type MarketPage,
  type MarketSkill,
  type MarketSkillDetail,
  type MarketSort,
  type McpServerView,
  type Overview,
  type ProjectSummary,
  type ApplyReport,
  type ProbeInput,
  type ProbeResult,
  type ProviderInput,
  type ProvidersView,
  type SupplierDef,
  type SettingsInput,
  type SettingsView,
  type UsageRange,
  type UsageReport,
  type WatchDelta,
} from "./types";
import { autoRouteId, defaultRetry, directBlockReason, routeProtocolFor } from "./supplier";
import type { ModelSlots, PoolMode } from "./types";

/** 与 src-tauri/src/watch.rs 的 EVENT 常量保持一致。 */
const FS_CHANGED_EVENT = "hub://fs-changed";

interface HubState {
  overview: Overview | null;
  instructionData: InstructionOverview;
  projects: ProjectSummary[];
  /** 常见 Agent 目录；老壳没有该命令时为空数组，页面回退到 overview.report.agents */
  catalog: CatalogEntry[];
  catalogLoading: boolean;
  /** 设置页数据；null = 还没读过 */
  settings: SettingsView | null;
  settingsLoading: boolean;
  refreshSettings: () => Promise<void>;
  /** 保存成功后自动重扫；返回是否成功（失败时 toast 已带原因） */
  saveSettings: (input: SettingsInput) => Promise<boolean>;
  /** Phase 3：provider 增删改与绑定；每次成功都会把最新视图写回 settings.providers */
  saveProvider: (input: ProviderInput) => Promise<boolean>;
  deleteProvider: (id: string) => Promise<void>;
  bindProvider: (input: BindProviderInput) => Promise<boolean>;
  unbindProvider: (agent: string) => Promise<void>;
  /** 生成写入计划并打开计划抽屉；失败时 toast 带原因 */
  planProviderApply: (agent: string) => Promise<void>;
  /** 功能开关：导航、命令面板、右键菜单都按它过滤。默认全开 */
  features: FeatureFlags;
  /** 翻转一个功能开关（即时生效，不进 Plan/Apply） */
  setFeature: (key: FeatureKey, enabled: boolean) => Promise<boolean>;
  /** 最近一次「一键应用」的结果，按 agent slug 索引；切换器用它显示状态灯 */
  applyReports: Record<string, ApplyReport>;
  /** 一键应用：写入 + 探测一次，返回结构化结果（失败返回 null 并 toast） */
  applyProviderBinding: (agent: string) => Promise<ApplyReport | null>;
  /** 把供应商加进该 agent 的池（池为空时自动成为默认） */
  addToPool: (agent: string, providerId: string) => Promise<ApplyReport | null>;
  /** 从池里移除；删掉的是默认时自动把剩余第一个顶上 */
  removeFromPool: (agent: string, providerId: string) => Promise<ApplyReport | null>;
  /** 设为默认供应商（同时保证它在池里） */
  setDefaultProvider: (agent: string, providerId: string) => Promise<ApplyReport | null>;
  /** 切换三模式：直连 / 路由 / 聚合 */
  setPoolMode: (agent: string, mode: PoolMode) => Promise<ApplyReport | null>;
  /** 只更新 Claude 的四个模型槽位 */
  setModelSlots: (agent: string, slots: ModelSlots) => Promise<ApplyReport | null>;
  /** 内置供应商目录（静态表，进页面时拉一次） */
  suppliers: SupplierDef[];
  loadSuppliers: (keyword?: string) => Promise<void>;
  /** 拉上游模型列表；失败时返回的 ProbeResult 里带 error，不抛异常 */
  probeModels: (input: ProbeInput) => Promise<ProbeResult | null>;
  /** 网关配置与路由；null = 还没读过 */
  gateway: GatewayView | null;
  gatewayStatus: GatewayStatusView | null;
  gatewayLogs: GatewayRequestLog[];
  gatewayLoading: boolean;
  refreshGateway: () => Promise<void>;
  refreshGatewayStatus: () => Promise<void>;
  refreshGatewayLogs: (limit?: number) => Promise<void>;
  saveGatewayConfig: (input: GatewayConfigInput) => Promise<boolean>;
  saveGatewayRoute: (route: GatewayRouteView) => Promise<boolean>;
  deleteGatewayRoute: (id: string) => Promise<boolean>;
  startGateway: () => Promise<void>;
  stopGateway: () => Promise<void>;
  restartGateway: () => Promise<void>;
  clearGatewayLogs: () => Promise<void>;
  /** 把经网关的 agent 退回直连的写入计划 */
  planRevertToDirect: (agents: string[]) => Promise<void>;
  plan: ChangeSet | null;
  /** 当前计划所属的项目作用域（null = 全局）；apply 时要用同一个作用域 */
  planProject: string | null;
  loading: boolean;
  applying: boolean;
  error: string | null;
  toast: string | null;
  realtime: boolean;
  /** 外部工具改了受管目录时，壳层推来的变化摘要；null 表示没有待处理提示 */
  fsChange: WatchDelta | null;
  dismissFsChange: () => void;
  /** 提示条「重扫」：清提示 + 重扫 + toast 反馈 */
  rescan: () => Promise<void>;
  refresh: () => Promise<void>;
  /** probe=true 会短暂启动 CLI 读版本；只有用户点击时才传 true */
  refreshCatalog: (probe?: boolean) => Promise<void>;
  addManagedAgent: (key: string) => Promise<void>;
  removeManagedAgent: (key: string) => Promise<void>;
  setPlan: (plan: ChangeSet | null) => void;
  planDedupe: (only?: string[], baseline?: string | null) => Promise<void>;
  planRepair: () => Promise<void>;
  /** 删除技能：生成计划并打开抽屉（人工确认发生在弹窗 + 抽屉两处） */
  planDelete: (paths: string[], opts: DeleteOptions) => Promise<void>;
  /** 市场三件事只做查询，错误抛给调用方在弹层里就地显示 */
  marketSearch: (
    query: string,
    sort: MarketSort,
    cursor?: string | null,
    limit?: number
  ) => Promise<MarketPage>;
  marketDetail: (slug: string) => Promise<MarketSkillDetail>;
  marketAgents: () => Promise<MarketAgent[]>;
  /** 生成安装计划；成功返回 true（弹层据此关闭） */
  planMarketInstall: (input: MarketInstallInput) => Promise<boolean>;
  mcpView: (project: string | null) => Promise<McpServerView[]>;
  planMcpWrite: (
    project: string | null,
    servers: McpServerView[],
    agents: string[]
  ) => Promise<void>;
  applyPlan: () => Promise<void>;
  rollback: (batch: string) => Promise<void>;
  setSkillStrategy: (
    name: string,
    strategy: LinkStrategyName | null
  ) => Promise<void>;
  /** 标记 / 取消「agent 独有技能」：不参与去重接管与链接修复，落盘后自动重扫 */
  setAgentLocalSkill: (name: string, on: boolean) => Promise<void>;
  /** Phase 4：token 用量报告。null = 还没统计过（首次进入用量页才拉） */
  usage: UsageReport | null;
  usageLoading: boolean;
  /** `force = true` 丢弃按文件缓存强制重算 */
  refreshUsage: (range: UsageRange, force?: boolean) => Promise<void>;
  notify: (message: string) => void;
}

const HubContext = createContext<HubState | null>(null);

export function HubProvider({ children }: { children: ReactNode }) {
  const [overview, setOverview] = useState<Overview | null>(null);
  const [instructionData, setInstructionData] = useState<InstructionOverview>({
    specs: [],
    chain: [],
  });
  const [projects, setProjects] = useState<ProjectSummary[]>([]);
  const [catalog, setCatalog] = useState<CatalogEntry[]>([]);
  const [catalogLoading, setCatalogLoading] = useState(false);
  const [settings, setSettings] = useState<SettingsView | null>(null);
  const [settingsLoading, setSettingsLoading] = useState(false);
  const [plan, setPlan] = useState<ChangeSet | null>(null);
  const [planProject, setPlanProject] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [applying, setApplying] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [toast, setToast] = useState<string | null>(null);
  const [fsChange, setFsChange] = useState<WatchDelta | null>(null);
  const [usage, setUsage] = useState<UsageReport | null>(null);
  const [usageLoading, setUsageLoading] = useState(false);
  const [features, setFeatures] = useState<FeatureFlags>(DEFAULT_FEATURES);
  const [gateway, setGateway] = useState<GatewayView | null>(null);
  const [gatewayStatus, setGatewayStatus] = useState<GatewayStatusView | null>(null);
  const [gatewayLogs, setGatewayLogs] = useState<GatewayRequestLog[]>([]);
  const [gatewayLoading, setGatewayLoading] = useState(false);
  const [suppliers, setSuppliers] = useState<SupplierDef[]>([]);
  const [applyReports, setApplyReports] = useState<Record<string, ApplyReport>>({});

  const notify = useCallback((message: string) => {
    setToast(message);
    window.setTimeout(() => setToast(null), 3200);
  }, []);

  /**
   * 失败提示统一走一句模板：动作名 + 原始原因。
   *
   * 三十来处「XXX 失败：…」如果各写一条，中英句式会各写各的；这里把动作名抽成
   * `toast.act.*`，英文用「Failed to {action}: {error}」，一处模板管到底。
   */
  const fail = useCallback(
    (action: MessageKey, err: unknown) =>
      notify(
        tr("toast.failed", {
          action: tr(action),
          error: err instanceof Error ? err.message : String(err),
        })
      ),
    [notify]
  );

  /**
   * 只刷新目录（不重扫技能目录）。`probe` 为 true 时才起子进程读版本。
   * 失败不清空旧数据：老版本壳没有这条命令时页面会回退到 overview.report.agents。
   */
  const refreshCatalog = useCallback(async (probe = false) => {
    setCatalogLoading(true);
    try {
      const next = await api.agentCatalog(probe);
      setCatalog(next);
    } catch (err) {
      console.error("读取 Agent 目录失败", err);
    } finally {
      setCatalogLoading(false);
    }
  }, []);

  /** 只读设置（不重扫技能目录）。 */
  const refreshSettings = useCallback(async () => {
    setSettingsLoading(true);
    try {
      const next = await api.settings();
      setSettings(next);
      // 设置页也能改开关，顺手同步一份，避免两处状态打架
      if (next.features) setFeatures(next.features);
    } catch (err) {
      console.error("读取设置失败", err);
    } finally {
      setSettingsLoading(false);
    }
  }, []);

  /**
   * 功能开关单独拉：导航过滤在首帧就要结果，不能等整个设置视图
   * （它会统计仓库占用）。失败时保持默认全开，绝不把模块误藏起来。
   */
  useEffect(() => {
    let disposed = false;
    void api
      .features()
      .then((next) => {
        if (!disposed && next) setFeatures(next);
      })
      .catch((err) => {
        console.warn("读取功能开关失败，按全开处理", err);
      });
    return () => {
      disposed = true;
    };
  }, []);

  const setFeature = useCallback(
    async (key: FeatureKey, enabled: boolean) => {
      try {
        const next = await api.setFeature(key, enabled);
        setFeatures(next);
        setSettings((prev) => (prev ? { ...prev, features: next } : prev));
        const name =
          key === "provider" ? tr("common.feature.provider") : tr("common.feature.usage");
        notify(enabled ? tr("toast.featureOn", { name }) : tr("toast.featureOff", { name }));
        return true;
      } catch (err) {
        fail("toast.act.toggleFeature", err);
        return false;
      }
    },
    [fail, notify]
  );

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [ov, ins, proj] = await Promise.all([
        api.overview(),
        api.instructions(),
        api.projects(),
      ]);
      setOverview(ov);
      setInstructionData(ins);
      setProjects(proj);
      // 目录检测很轻（只读 PATH/目录），但失败不能拖垮主流程
      void refreshCatalog(false);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, [refreshCatalog]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  // 外部变更只做「提示」，不自动重扫：自动重扫会打断当前筛选/选中态
  useEffect(() => {
    if (!isTauri) return;
    let disposed = false;
    let unlisten: (() => void) | null = null;

    void listen<WatchDelta>(FS_CHANGED_EVENT, (event) => {
      const delta = event.payload;
      if (!delta) return;
      if (delta.added.length + delta.removed.length + delta.changed.length === 0) return;
      // 合并而不是覆盖：一次编辑会连发多条事件，覆盖会让提示条不停地「闪同一个数」
      setFsChange((prev) => mergeDelta(prev, delta));
    })
      .then((fn) => {
        if (disposed) fn();
        else unlisten = fn;
      })
      .catch((err) => {
        // 事件通道不可用（老版本壳 / 缺 capabilities）时降级为手动刷新。
        // 别再静默吞掉：这条 catch 曾经把「Tauri ACL 拒了 listen」藏了一整天。
        console.warn("外部变更事件通道不可用，将退回手动重新扫描", err);
      });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  const dismissFsChange = useCallback(() => setFsChange(null), []);

  /**
   * 提示条上的「重扫」：扫描前后各清一次，最后给一句反馈。
   *
   * 两次清空都有必要：
   * - 扫描**前**清，提示条立刻消失，用户能确认点击生效了；
   * - 扫描**后**再清，把扫描窗口（上百毫秒）内到达的事件抹掉——那些改动已经被这次
   *   扫描结果覆盖了，留着会变成「刚点完就又冒出来」，看起来像重扫没生效。
   * 数据根不在监听列表里（见 `AgentHub::watch_targets`），扫描写索引库不会再触发事件。
   */
  const rescan = useCallback(async () => {
    setFsChange(null);
    await refresh();
    setFsChange(null);
    notify(tr("toast.rescanned"));
  }, [notify, refresh]);

  const planDedupe = useCallback(
    async (only?: string[], baseline?: string | null) => {
      try {
        const next = await api.planDedupe(only, baseline);
        setPlanProject(null);
        setPlan(next);
        // 空计划优先回显后端 note：agent 独有技能被显式点名时，原因就在第一条
        if (next.ops.length === 0) notify(next.notes[0] ?? tr("toast.nothingToProcess"));
      } catch (err) {
        notify(err instanceof Error ? err.message : String(err));
      }
    },
    [notify]
  );

  const planRepair = useCallback(async () => {
    try {
      const next = await api.planRepair();
      setPlanProject(null);
      setPlan(next);
      if (next.ops.length === 0) notify(tr("toast.allLinksHealthy"));
    } catch (err) {
      notify(err instanceof Error ? err.message : String(err));
    }
  }, [notify]);

  const mcpView = useCallback(
    (project: string | null) => api.mcpView(project),
    []
  );

  /** 删除计划：与其它计划一样落到同一个抽屉，执行前用户还能逐条看 op。 */
  const planDelete = useCallback(
    async (paths: string[], opts: DeleteOptions) => {
      try {
        const next = await api.planDelete(paths, opts);
        setPlanProject(null);
        setPlan(next);
        if (next.ops.length === 0) {
          notify(next.notes[0] ?? tr("toast.nothingToDelete"));
          setPlan(null);
        }
      } catch (err) {
        fail("toast.act.deletePlan", err);
      }
    },
    [fail, notify]
  );

  const marketSearch = useCallback(
    (query: string, sort: MarketSort, cursor?: string | null, limit?: number) =>
      api.marketSearch(query, sort, cursor, limit),
    []
  );

  const marketDetail = useCallback((slug: string) => api.marketDetail(slug), []);

  const marketAgents = useCallback(() => api.marketAgents(), []);

  const planMarketInstall = useCallback(
    async (input: MarketInstallInput) => {
      try {
        const next = await api.planMarketInstall(input);
        setPlanProject(null);
        setPlan(next);
        if (next.ops.length === 0) {
          notify(tr("toast.nothingToInstall"));
          setPlan(null);
          return false;
        }
        return true;
      } catch (err) {
        fail("toast.act.install", err);
        return false;
      }
    },
    [fail, notify]
  );

  const planMcpWrite = useCallback(
    async (
      project: string | null,
      servers: McpServerView[],
      agents: string[]
    ) => {
      try {
        const next = await api.planMcpWrite(project, servers, agents);
        setPlanProject(project);
        setPlan(next);
        if (next.ops.length === 0) notify(tr("toast.nothingToWrite"));
      } catch (err) {
        fail("toast.act.writePlan", err);
      }
    },
    [fail, notify]
  );

  const applyPlan = useCallback(async () => {
    if (!plan) return;
    setApplying(true);
    try {
      // 项目作用域的计划必须用同一作用域执行，否则写入门禁会拒绝项目内路径
      const result = await api.apply(plan, planProject);
      setPlan(null);
      setPlanProject(null);
      await refresh();
      // 这批写盘是我们自己干的：刷新后把「外部变更」提示清掉，别让用户以为有第三方改动
      setFsChange(null);
      notify(tr("toast.applied", { count: result.applied, batch: result.batch_id }));
    } catch (err) {
      fail("toast.act.apply", err);
    } finally {
      setApplying(false);
    }
  }, [plan, planProject, fail, notify, refresh]);

  const rollback = useCallback(
    async (batch: string) => {
      try {
        await api.rollback(batch);
        await refresh();
        setFsChange(null);
        notify(tr("toast.rolledBack", { batch }));
      } catch (err) {
        fail("toast.act.rollback", err);
      }
    },
    [fail, notify, refresh]
  );

  /**
   * 单条技能链接策略：落库后只更新 overview 里的策略解析结果，
   * 不触发整页重扫（重扫会打断当前筛选与选中态）。
   */
  const setSkillStrategy = useCallback(
    async (name: string, strategy: LinkStrategyName | null) => {
      try {
        const next = await api.setSkillStrategy(name, strategy);
        setOverview((prev) => (prev ? { ...prev, strategies: next } : prev));
        notify(
          strategy
            ? tr("toast.strategySet", { name, strategy: strategyLabel(strategy) })
            : tr("toast.strategyAuto", { name, strategy: strategyLabel(next.default) })
        );
      } catch (err) {
        fail("toast.act.setStrategy", err);
      }
    },
    [fail, notify]
  );

  /**
   * 标记 / 取消「agent 独有技能」（computer-use 这类各 agent 实现互不通用的技能）。
   *
   * 只改 `config.json`，但**必须重扫**：`agent_local` 是扫描期按名单打的标记，
   * 只改本地 state 会让列表徽标与下一次去重/修复计划对不上。重扫不动筛选与选中态。
   */
  const setAgentLocalSkill = useCallback(
    async (name: string, on: boolean) => {
      try {
        await api.setAgentLocalSkill(name, on);
        await refresh();
        notify(on ? tr("toast.agentLocalOn", { name }) : tr("toast.agentLocalOff", { name }));
      } catch (err) {
        fail("toast.act.agentLocal", err);
      }
    },
    [fail, notify, refresh]
  );

  const saveSettings = useCallback(
    async (input: SettingsInput) => {
      try {
        const next = await api.saveSettings(input);
        setSettings(next);
        notify(tr("toast.settingsSaved"));
        // 路径覆盖与自定义平台都是「装配期」生效，必须重扫才能看到变化
        await refresh();
        return true;
      } catch (err) {
        fail("toast.act.save", err);
        return false;
      }
    },
    [fail, notify, refresh]
  );

  const addManagedAgent = useCallback(
    async (key: string) => {
      try {
        const next = await api.addManagedAgent(key);
        setCatalog(next);
        const name = next.find((entry) => entry.key === key)?.name ?? key;
        notify(tr("toast.agentManaged", { name }));
        // 新平台要进入技能扫描与 MCP 分发，重扫一次拿最新视图
        await refresh();
      } catch (err) {
        fail("toast.act.manageAgent", err);
      }
    },
    [fail, notify, refresh]
  );

  /** provider 的四个写操作都把最新视图塞回 `settings.providers`（设置页一次读全）。 */
  const applyProviders = useCallback((next: ProvidersView) => {
    setSettings((prev) => (prev ? { ...prev, providers: next } : prev));
  }, []);

  const saveProvider = useCallback(
    async (input: ProviderInput) => {
      try {
        applyProviders(await api.saveProvider(input));
        notify(tr("toast.providerSaved", { name: input.name }));
        return true;
      } catch (err) {
        fail("toast.act.saveProvider", err);
        return false;
      }
    },
    [applyProviders, fail, notify]
  );

  const deleteProvider = useCallback(
    async (id: string) => {
      try {
        applyProviders(await api.deleteProvider(id));
        notify(tr("toast.providerDeleted", { id }));
      } catch (err) {
        fail("toast.act.deleteProvider", err);
      }
    },
    [applyProviders, fail, notify]
  );

  const bindProvider = useCallback(
    async (input: BindProviderInput) => {
      try {
        applyProviders(await api.bindProvider(input));
        notify(tr("toast.bound", { agent: input.agent, provider: input.provider_id }));
        return true;
      } catch (err) {
        fail("toast.act.bind", err);
        return false;
      }
    },
    [applyProviders, fail, notify]
  );

  const unbindProvider = useCallback(
    async (agent: string) => {
      try {
        applyProviders(await api.unbindProvider(agent));
        notify(tr("toast.unbound", { agent }));
      } catch (err) {
        fail("toast.act.unbind", err);
      }
    },
    [applyProviders, fail, notify]
  );

  const planProviderApply = useCallback(
    async (agent: string) => {
      try {
        const next = await api.planProviderApply(agent);
        setPlanProject(null);
        setPlan(next);
        if (next.ops.length === 0) notify(tr("toast.upToDate"));
      } catch (err) {
        fail("toast.act.plan", err);
      }
    },
    [fail, notify]
  );

  const applyProviderBinding = useCallback(
    async (agent: string) => {
      try {
        const report = await api.applyProviderBinding(agent);
        setApplyReports((prev) => ({ ...prev, [agent]: report }));
        // 写入可能改了绑定里的模型（比如供应商模型列表变了），顺带刷一次设置视图
        void refreshSettings();
        return report;
      } catch (err) {
        fail("toast.act.apply", err);
        return null;
      }
    },
    [fail, notify, refreshSettings]
  );

  /**
   * 点一个供应商 chip 的完整动作。
   *
   * 直连能成立就直连；不能（Claude 接 Chat 上游）就自动走网关：
   * 找一条现成路由，没有就按需建一条 `auto-<agent>-<provider>`，再启动网关。
   * 用户不需要知道「路由」是什么。
   */
  /**
   * 四个池动作共用一套收尾：记下最近一次报告、刷新设置视图。
   *
   * 每一步在后端都是「改 config → 写 agent 配置 → 探测」，前端不需要再点「应用」。
   */
  const runPoolAction = useCallback(
    async (agent: string, run: () => Promise<ApplyReport>) => {
      try {
        const report = await run();
        setApplyReports((prev) => ({ ...prev, [agent]: report }));
        await refreshSettings();
        return report;
      } catch (err) {
        fail("toast.act.pool", err);
        return null;
      }
    },
    [fail, notify, refreshSettings]
  );

  const addToPool = useCallback(
    (agent: string, providerId: string) =>
      runPoolAction(agent, () => api.addToPool(agent, providerId)),
    [runPoolAction]
  );

  const removeFromPool = useCallback(
    (agent: string, providerId: string) =>
      runPoolAction(agent, () => api.removeFromPool(agent, providerId)),
    [runPoolAction]
  );

  const setDefaultProvider = useCallback(
    (agent: string, providerId: string) =>
      runPoolAction(agent, () => api.setDefaultProvider(agent, providerId)),
    [runPoolAction]
  );

  const setPoolMode = useCallback(
    (agent: string, mode: PoolMode) =>
      runPoolAction(agent, () => api.setPoolMode(agent, mode)),
    [runPoolAction]
  );

  const setModelSlots = useCallback(
    (agent: string, slots: ModelSlots) =>
      runPoolAction(agent, () => api.setModelSlots(agent, slots)),
    [runPoolAction]
  );

  const loadSuppliers = useCallback(
    async (keyword = "") => {
      try {
        setSuppliers(await api.suppliers(keyword));
      } catch (err) {
        fail("toast.act.loadSuppliers", err);
      }
    },
    [fail, notify]
  );

  const probeModels = useCallback(
    async (input: ProbeInput) => {
      try {
        return await api.probeProviderModels(input);
      } catch (err) {
        fail("toast.act.probeModels", err);
        return null;
      }
    },
    [fail, notify]
  );

  const refreshGateway = useCallback(async () => {
    setGatewayLoading(true);
    try {
      setGateway(await api.gateway());
    } catch (err) {
      fail("toast.act.readGateway", err);
    } finally {
      setGatewayLoading(false);
    }
  }, [fail, notify]);

  const refreshGatewayStatus = useCallback(async () => {
    try {
      setGatewayStatus(await api.gatewayStatus());
    } catch (err) {
      console.error("读取网关状态失败", err);
    }
  }, []);

  const refreshGatewayLogs = useCallback(async (limit = 200) => {
    try {
      setGatewayLogs(await api.gatewayLogs(limit));
    } catch (err) {
      console.error("读取网关请求日志失败", err);
    }
  }, []);

  const saveGatewayConfig = useCallback(
    async (input: GatewayConfigInput) => {
      try {
        setGateway(await api.saveGatewayConfig(input));
        setGatewayStatus(await api.gatewayStatus());
        notify(tr("toast.gatewaySaved"));
        return true;
      } catch (err) {
        fail("toast.act.saveGateway", err);
        return false;
      }
    },
    [fail, notify]
  );

  const saveGatewayRoute = useCallback(
    async (route: GatewayRouteView) => {
      try {
        setGateway(await api.saveGatewayRoute(route));
        setGatewayStatus(await api.gatewayStatus());
        notify(tr("toast.routeSaved", { name: route.name }));
        return true;
      } catch (err) {
        fail("toast.act.saveRoute", err);
        return false;
      }
    },
    [fail, notify]
  );

  const deleteGatewayRoute = useCallback(
    async (id: string) => {
      try {
        const result = await api.deleteGatewayRoute(id);
        setGateway(result.view);
        setGatewayStatus(await api.gatewayStatus());
        // 绑定被退回直连了：刷新一次设置视图，绑定区才会显示「直连」
        await refreshSettings();
        notify(
          result.reverted.length
            ? tr("toast.routeDeletedReverted", {
                id,
                agents: result.reverted.join(" / "),
              })
            : tr("toast.routeDeleted", { id })
        );
        return true;
      } catch (err) {
        fail("toast.act.deleteRoute", err);
        return false;
      }
    },
    [fail, notify, refreshSettings]
  );

  const startGateway = useCallback(async () => {
    try {
      setGatewayStatus(await api.gatewayStart());
      notify(tr("toast.gatewayStarted"));
    } catch (err) {
      fail("toast.act.startGateway", err);
      await refreshGatewayStatus();
    }
  }, [fail, notify, refreshGatewayStatus]);

  const stopGateway = useCallback(async () => {
    try {
      setGatewayStatus(await api.gatewayStop());
      notify(tr("toast.gatewayStopped"));
    } catch (err) {
      fail("toast.act.stopGateway", err);
    }
  }, [fail, notify]);

  const restartGateway = useCallback(async () => {
    try {
      setGatewayStatus(await api.gatewayRestart());
      notify(tr("toast.gatewayRestarted"));
    } catch (err) {
      fail("toast.act.restartGateway", err);
    }
  }, [fail, notify]);

  const clearGatewayLogs = useCallback(async () => {
    try {
      await api.clearGatewayLogs();
      setGatewayLogs([]);
    } catch (err) {
      fail("toast.act.clear", err);
    }
  }, [fail, notify]);

  const planRevertToDirect = useCallback(
    async (agents: string[]) => {
      try {
        const next = await api.planRevertToDirect(agents);
        setPlanProject(null);
        setPlan(next);
        if (next.ops.length === 0) notify(tr("toast.noGatewayBindings"));
      } catch (err) {
        fail("toast.act.revertPlan", err);
      }
    },
    [fail, notify]
  );

  /**
   * token 用量：刻意**不放进启动 refresh**。
   *
   * 首次统计要流式读几百 MB 会话日志（实测 320 文件 / 约 0.8 s，但机器慢时会到数秒），
   * 挂在启动路径上等于每次开窗都付一次代价。只由用量页在挂载 / 切换范围时调用。
   */
  const refreshUsage = useCallback(
    async (range: UsageRange, force = false) => {
      setUsageLoading(true);
      try {
        setUsage(await api.usage(range, force));
      } catch (err) {
        fail("toast.act.usage", err);
      } finally {
        setUsageLoading(false);
      }
    },
    [fail, notify]
  );

  const removeManagedAgent = useCallback(
    async (key: string) => {
      try {
        const next = await api.removeManagedAgent(key);
        setCatalog(next);
        const name = next.find((entry) => entry.key === key)?.name ?? key;
        notify(tr("toast.agentUnmanaged", { name }));
        await refresh();
      } catch (err) {
        fail("toast.act.unmanageAgent", err);
      }
    },
    [fail, notify, refresh]
  );

  const value = useMemo<HubState>(
    () => ({
      overview,
      instructionData,
      projects,
      catalog,
      catalogLoading,
      settings,
      settingsLoading,
      plan,
      planProject,
      loading,
      applying,
      error,
      toast,
      realtime: isTauri,
      fsChange,
      dismissFsChange,
      rescan,
      refresh,
      refreshCatalog,
      refreshSettings,
      saveSettings,
      saveProvider,
      deleteProvider,
      bindProvider,
      unbindProvider,
      planProviderApply,
      applyReports,
      applyProviderBinding,
      addToPool,
      removeFromPool,
      setDefaultProvider,
      setPoolMode,
      setModelSlots,
      suppliers,
      loadSuppliers,
      probeModels,
      features,
      setFeature,
      gateway,
      gatewayStatus,
      gatewayLogs,
      gatewayLoading,
      refreshGateway,
      refreshGatewayStatus,
      refreshGatewayLogs,
      saveGatewayConfig,
      saveGatewayRoute,
      deleteGatewayRoute,
      startGateway,
      stopGateway,
      restartGateway,
      clearGatewayLogs,
      planRevertToDirect,
      addManagedAgent,
      removeManagedAgent,
      setPlan,
      planDedupe,
      planRepair,
      planDelete,
      marketSearch,
      marketDetail,
      marketAgents,
      planMarketInstall,
      mcpView,
      planMcpWrite,
      applyPlan,
      rollback,
      setSkillStrategy,
      setAgentLocalSkill,
      usage,
      usageLoading,
      refreshUsage,
      notify,
    }),
    [
      overview,
      instructionData,
      projects,
      catalog,
      catalogLoading,
      settings,
      settingsLoading,
      plan,
      planProject,
      loading,
      applying,
      error,
      toast,
      fsChange,
      dismissFsChange,
      rescan,
      refresh,
      refreshCatalog,
      refreshSettings,
      saveSettings,
      saveProvider,
      deleteProvider,
      bindProvider,
      unbindProvider,
      planProviderApply,
      applyReports,
      applyProviderBinding,
      addToPool,
      removeFromPool,
      setDefaultProvider,
      setPoolMode,
      setModelSlots,
      suppliers,
      loadSuppliers,
      probeModels,
      features,
      setFeature,
      gateway,
      gatewayStatus,
      gatewayLogs,
      gatewayLoading,
      refreshGateway,
      refreshGatewayStatus,
      refreshGatewayLogs,
      saveGatewayConfig,
      saveGatewayRoute,
      deleteGatewayRoute,
      startGateway,
      stopGateway,
      restartGateway,
      clearGatewayLogs,
      planRevertToDirect,
      addManagedAgent,
      removeManagedAgent,
      planDedupe,
      planRepair,
      planDelete,
      marketSearch,
      marketDetail,
      marketAgents,
      planMarketInstall,
      mcpView,
      planMcpWrite,
      applyPlan,
      rollback,
      setSkillStrategy,
      setAgentLocalSkill,
      usage,
      usageLoading,
      refreshUsage,
      notify,
    ]
  );

  return <HubContext.Provider value={value}>{children}</HubContext.Provider>;
}

export function useHub(): HubState {
  const ctx = useContext(HubContext);
  if (!ctx) {
    throw new Error("useHub 必须在 HubProvider 内使用");
  }
  return ctx;
}
