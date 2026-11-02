import { lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { clsx } from "clsx";
import { Inspector } from "./components/workbench/Inspector";
import { NavRail } from "./components/workbench/NavRail";
import { StatusBar } from "./components/workbench/StatusBar";
import { Toolbar } from "./components/workbench/Toolbar";
import { CommandPalette } from "./components/workbench/CommandPalette";
import { ContextMenuHost } from "./components/workbench/ContextMenuHost";
import { SkillDiffDialog } from "./components/skills/SkillDiffDialog";
import { PlanDrawer } from "./components/PlanDrawer";
import { HubProvider, useHub } from "./lib/useHub";
import { LocaleProvider, useLocale, useLocaleTag, useT } from "./lib/i18n";
import { ThemeProvider, useTheme } from "./lib/theme";
import { SelectionProvider, useSelection } from "./lib/selection";
import { PaletteProvider, usePalette } from "./lib/palette";
import {
  ContextMenuProvider,
  MenuEnvProvider,
  useContextMenu,
  type MenuEnv,
} from "./lib/context-menu";
import { useHotkeys, type Hotkey } from "./lib/hotkeys";
import { buildCommands } from "./lib/commands";
import { buildHash, parseHash, type Route, type RouteParams } from "./lib/route";
import {
  INSPECTOR_OPEN_KEY,
  NAV_COLLAPSED_KEY,
  useLayoutFlag,
} from "./lib/layout";
import { featureOf, navShortcut, visibleNav, type NavKey } from "./lib/nav";
import { basename } from "./lib/format";
import { Agents } from "./pages/Agents";
import { Provider } from "./pages/Provider";
import { Usage } from "./pages/Usage";
import { Dashboard } from "./pages/Dashboard";
import { FeatureDisabled } from "./components/common/FeatureDisabled";

/**
 * 终端页按需加载：xterm.js 加上 fit / web-links 插件约 300KB，
 * 不该让「打开就是总览」的启动路径替它买单。
 */
const Terminal = lazy(() =>
  import("./pages/Terminal").then((module) => ({ default: module.Terminal }))
);
import { Skills } from "./pages/Skills";
import { Mcp } from "./pages/Mcp";
import { Instructions } from "./pages/Instructions";
import { Projects } from "./pages/Projects";
import { Changes } from "./pages/Changes";
import { Settings } from "./pages/Settings";
import type { TerminalOpenRequest } from "./pages/Terminal";
import type { LinkTarget } from "./lib/terminal-links";

function Console() {
  const [route, setRoute] = useState<Route>(() => parseHash());
  const [diffSkill, setDiffSkill] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [skillsAgent, setSkillsAgent] = useState<string | null>(null);
  /** 打开 Agent 环境时要定位的产品 key（检查器 / 命令面板传入） */
  const [agentsFocus, setAgentsFocus] = useState<string | null>(null);
  // 窗口最小 1100px：窄屏默认把导航收成图标栏，把宽度让给画布
  const [navCollapsed, , toggleNav] = useLayoutFlag(
    NAV_COLLAPSED_KEY,
    window.innerWidth < 1280
  );
  const [inspectorOpen, setInspectorOpen, toggleInspector] = useLayoutFlag(
    INSPECTOR_OPEN_KEY,
    window.innerWidth >= 1280
  );
  /**
   * 终端页首次进入后常驻挂载（其它模块下只是 `hidden`）：
   * 切去技能库看一眼不应该把正在跑的 agent 进程连带杀掉。
   */
  const [terminalMounted, setTerminalMounted] = useState(route.page === "terminal");

  const {
    overview,
    loading,
    error,
    toast,
    refresh,
    rollback,
    plan,
    setPlan,
    planDedupe,
    planRepair,
    applyPlan,
    applying,
    projects,
    catalog,
    features,
    gatewayStatus,
    startGateway,
    stopGateway,
  } = useHub();
  const { cycle } = useTheme();
  const t = useT();
  const { revision } = useLocale();
  const localeTag = useLocaleTag();
  const { selection, clear } = useSelection();
  const palette = usePalette();
  const contextMenu = useContextMenu();

  const active = route.page;
  /**
   * 当前页是否属于一个被关闭的功能。
   *
   * 导航里已经不列了，但 hash 可以被手输 / 旧书签带进来，这时给一张说明页而不是
   * 渲染一个后端会拒绝所有命令的空页面。
   */
  const blockedFeature = (() => {
    const key = featureOf(active);
    return key && !features[key] ? key : null;
  })();

  useEffect(() => {
    const onHash = () => setRoute(parseHash());
    window.addEventListener("hashchange", onHash);
    return () => window.removeEventListener("hashchange", onHash);
  }, []);

  useEffect(() => {
    if (active === "terminal") setTerminalMounted(true);
  }, [active]);

  const navigate = useCallback((key: NavKey, params: RouteParams = {}) => {
    const hash = buildHash(key, params);
    if (window.location.hash !== hash) window.location.hash = hash;
    setRoute({ page: key, batch: params.batch ?? null, cwd: params.cwd ?? null });
    if (key !== "skills") setSearch("");
  }, []);

  /**
   * 「在终端打开」请求。cwd 同时写进 URL（可分享 / 刷新复现），
   * command / title 只在本进程内有效，走 state。
   */
  const [terminalOpen, setTerminalOpen] = useState<TerminalOpenRequest | null>(() =>
    route.cwd ? { token: 1, cwd: route.cwd } : null
  );
  const terminalToken = useRef(0);

  const openTerminal = useCallback(
    (
      cwd: string,
      extra?: { title?: string; command?: string | null; agentId?: string | null }
    ) => {
      terminalToken.current += 1;
      setTerminalOpen({ token: terminalToken.current, cwd, ...extra });
      navigate("terminal", { cwd });
    },
    [navigate]
  );

  /** 项目页要聚焦哪个仓库（从终端输出里的链接跳过来时） */
  const [projectsFocus, setProjectsFocus] = useState<string | null>(null);

  /** 终端输出里的链接：技能 / 项目 / MCP / 变更批次，各回各家 */
  const openTerminalLink = useCallback(
    (target: LinkTarget) => {
      if (target.kind === "skill") {
        setSearch(target.id);
        navigate("skills");
        return;
      }
      if (target.kind === "batch") {
        navigate("changes", { batch: target.id });
        return;
      }
      if (target.kind === "mcp") {
        navigate("mcp");
        return;
      }
      setProjectsFocus(target.id);
      navigate("projects");
    },
    [navigate]
  );

  /** 终端页消费完：清 state + 抹掉 URL 里的 cwd，免得刷新又开一个标签 */
  const consumeTerminalOpen = useCallback(() => {
    setTerminalOpen(null);
    if (!route.cwd) return;
    window.history.replaceState(null, "", buildHash(route.page, { batch: route.batch }));
    setRoute({ page: route.page, batch: route.batch, cwd: null });
  }, [route.batch, route.page, route.cwd]);

  const inspectAgent = useCallback(
    (agentId: string) => {
      setSkillsAgent(agentId || null);
      setSearch("");
      navigate("skills");
    },
    [navigate]
  );

  /**
   * 顶栏搜索：先跳到技能库再写入查询。
   * 顺序不能反——`navigate()` 在非 skills 页面会执行 `setSearch("")`，先写查询会被清掉。
   */
  const handleSearch = useCallback(
    (value: string) => {
      if (value && active !== "skills") navigate("skills");
      setSearch(value);
    },
    [active, navigate]
  );

  const openSkill = useCallback(
    (name: string) => {
      setSkillsAgent(null);
      setSearch(name);
      navigate("skills");
    },
    [navigate]
  );

  /**
   * 打开 Agent 环境并定位到某一行。检查器传的是内置 slug（claude / codex），
   * 而 catalog 的 key 是 claude-code / codex，所以先按 key 找、再按 builtin 找。
   */
  const openAgentEnv = useCallback(
    (key?: string | null) => {
      const target = key
        ? (catalog.find((entry) => entry.key === key) ??
            catalog.find((entry) => entry.builtin === key))?.key ?? key
        : null;
      setAgentsFocus(target);
      navigate("agents");
    },
    [catalog, navigate]
  );

  const agents = useMemo(
    () =>
      (overview?.report.agents ?? [])
        .filter((agent) => agent.detected)
        .map((agent) => ({ id: agent.id, name: agent.display_name })),
    [overview]
  );

  const skillNames = useMemo(
    () =>
      Array.from(new Set((overview?.report.records ?? []).map((record) => record.name))).sort(
        (a, b) => a.localeCompare(b, localeTag)
      ),
    [overview, localeTag]
  );

  const latestBatch = overview?.batches[0] ?? null;

  const commands = useMemo(
    () =>
      buildCommands({
        navigate,
        refresh: () => void refresh(),
        planDedupe: () => void planDedupe(),
        planRepair: () => void planRepair(),
        applyPlan: () => void applyPlan(),
        rollbackLatest: () => {
          if (latestBatch) void rollback(latestBatch.batch_id);
        },
        toggleTheme: cycle,
        toggleNav,
        toggleInspector,
        features,
        gatewayRunning: gatewayStatus?.running ?? false,
        startGateway: () => void startGateway(),
        stopGateway: () => void stopGateway(),
        openShortcuts: palette.openShortcuts,
        hasPlan: Boolean(plan),
        planOps: plan?.ops.length ?? 0,
        applying,
        latestBatch: latestBatch
          ? { id: latestBatch.batch_id, undone: latestBatch.undone }
          : null,
        skills: skillNames,
        projects: (projects ?? []).map((project) => ({ name: project.name, root: project.root })),
        agents,
        openSkill,
        openAgent: inspectAgent,
        catalog: catalog.map((entry) => ({
          key: entry.key,
          name: entry.name,
          hint:
            entry.managed === "builtin"
              ? t("agent.managed.builtin")
              : entry.managed === "managed"
                ? t("agent.managed.managed")
                : entry.state === "ready"
                  ? t("agent.managed.ready")
                  : t("agent.managed.missing"),
        })),
        openAgentEnv,
        skillDiffName: selection?.kind === "skill" ? basename(selection.id) : null,
        openSkillDiff: setDiffSkill,
        openTerminal,
      }),
    [
      navigate,
      refresh,
      planDedupe,
      planRepair,
      applyPlan,
      rollback,
      cycle,
      toggleNav,
      toggleInspector,
      features,
      gatewayStatus?.running,
      startGateway,
      stopGateway,
      palette.openShortcuts,
      plan,
      applying,
      latestBatch,
      skillNames,
      projects,
      agents,
      openSkill,
      inspectAgent,
      catalog,
      openAgentEnv,
      // 「对比同名技能」是否可用取决于当前选中项，漏了它命令会一直显示为置灰
      selection,
      // 命令表里的文案按语言生成：切换语言必须重建，否则标签停在旧语言
      revision,
      localeTag,
    ]
  );

  /** 页面构建右键菜单时需要的 App 级动作（行内闭包拿不到 navigate / openTerminal） */
  const menuEnv = useMemo<MenuEnv>(
    () => ({
      commands,
      openCommands: palette.openCommands,
      navigate,
      openTerminal: (cwd, extra) => openTerminal(cwd, extra),
      openSkill,
      openAgent: inspectAgent,
      openAgentEnv: (key) => openAgentEnv(key),
      openSkillDiff: setDiffSkill,
      skillDiffName: selection?.kind === "skill" ? basename(selection.id) : null,
    }),
    [commands, palette.openCommands, navigate, openTerminal, openSkill, inspectAgent, openAgentEnv, selection]
  );

  const handleEscape = useCallback(() => {
    // 菜单在最上层：先收它，避免一次 Esc 连带清选中 / 关抽屉
    if (contextMenu.state) {
      contextMenu.close();
      return;
    }
    if (palette.open) {
      palette.close();
      return;
    }
    if (diffSkill) {
      setDiffSkill(null);
      return;
    }
    if (plan) {
      setPlan(null);
      return;
    }
    if (selection) clear();
  }, [contextMenu, palette, diffSkill, plan, setPlan, selection, clear]);

  const hotkeys = useMemo<Hotkey[]>(
    () => [
      { combo: "mod+k", run: palette.toggleCommands },
      // 编号写死在 NavItem 上：新增模块不会顶掉既有 `mod+1..9`，也不会产生 `mod+10`。
      // 被禁用的模块连快捷键一起摘掉，按下去不该跳到一个「已禁用」页。
      ...visibleNav(features).flatMap((item) => {
        const combo = navShortcut(item.key);
        return combo ? [{ combo, run: () => navigate(item.key) }] : [];
      }),
      {
        combo: "mod+j",
        // 总览页与用量页的右栏已经承担了检查器职责，这里不再重复开关
        run: () => {
          if (active !== "dashboard" && active !== "usage") toggleInspector();
        },
      },
      { combo: "mod+b", run: toggleNav },
      { combo: "mod+shift+r", run: () => void refresh() },
      { combo: "?", run: palette.openShortcuts },
      { combo: "esc", allowInInput: true, run: handleEscape },
    ],
    [
      palette.toggleCommands,
      palette.openShortcuts,
      navigate,
      active,
      features,
      toggleInspector,
      toggleNav,
      refresh,
      handleEscape,
    ]
  );

  useHotkeys(hotkeys);

  const goRoot = useCallback(() => {
    clear();
    navigate(active);
  }, [clear, navigate, active]);

  /**
   * 顶栏待处理清单的「查看全部」：跳到总览页的问题卡片并短暂高亮。
   *
   * 延后一帧再滚：页面切换走 AnimatePresence，立刻取节点会拿到旧页面。
   */
  const showPendingIssues = useCallback(() => {
    navigate("dashboard");
    window.setTimeout(() => {
      const target = document.getElementById("pending-issues");
      if (!target) return;
      target.scrollIntoView({ block: "center", behavior: "smooth" });
      target.classList.add("ring-2", "ring-brand-500/50");
      window.setTimeout(
        () => target.classList.remove("ring-2", "ring-brand-500/50"),
        900
      );
    }, 120);
  }, [navigate]);

  return (
    <MenuEnvProvider value={menuEnv}>
    <div className="flex h-screen overflow-hidden bg-ink-950">
      <NavRail
        active={active}
        collapsed={navCollapsed}
        onToggle={toggleNav}
        onNavigate={(key) => navigate(key)}
      />

      <div className="flex min-w-0 flex-1 flex-col">
        <Toolbar
          active={active}
          onBack={goRoot}
          onOpenPalette={palette.openCommands}
          onRefresh={() => void refresh()}
          loading={loading}
          inspectorOpen={inspectorOpen}
          onToggleInspector={toggleInspector}
          // 总览 / 用量自带右栏、终端页整屏给 PTY、Provider 页自带请求流，
          // 都不需要常驻检查器
          showInspector={
            active !== "dashboard" &&
            active !== "terminal" &&
            active !== "usage" &&
            active !== "provider"
          }
          search={search}
          onSearch={handleSearch}
          onOpenSettings={() => navigate("settings")}
          onShowAllIssues={showPendingIssues}
        />

        <div className="flex min-h-0 flex-1">
          <main className="flex min-w-0 flex-1 flex-col bg-ink-950">
          {error && (
            <div className="border-b border-state-danger/30 bg-state-danger/10 px-3.5 py-1.5 text-[11.5px] text-state-danger">
              {t("app.error.config", { error })}
            </div>
          )}

          {/* 终端不依赖扫描结果：即使本机配置读取失败，也让人能开个 shell 排查 */}
          {(terminalMounted || active === "terminal") && (
            <div
              className={clsx(
                "min-h-0 flex-1 flex-col",
                active === "terminal" ? "flex" : "hidden"
              )}
            >
              <Suspense
                fallback={
                  <div className="flex flex-1 items-center justify-center text-[11.5px] text-text-dim">
                    {t("app.loadingTerminal")}
                  </div>
                }
              >
                <Terminal
                  openRequest={terminalOpen}
                  onConsumed={consumeTerminalOpen}
                  onOpenTarget={openTerminalLink}
                />
              </Suspense>
            </div>
          )}

          {active !== "terminal" && blockedFeature && (
            <FeatureDisabled
              page={active}
              onOpenSettings={() => navigate("settings")}
              onBack={() => navigate("dashboard")}
            />
          )}

          {active !== "terminal" &&
            !blockedFeature &&
            (!overview ? (
              <div className="flex flex-1 flex-col items-center justify-center gap-2 text-[11.5px] text-text-dim">
                <div className="h-6 w-6 animate-spin rounded-full border-2 border-line border-t-brand-500" />
                {t("app.scanning")}
              </div>
            ) : (
              <AnimatePresence mode="wait">
                <motion.div
                  key={active}
                  initial={{ opacity: 0, y: 4 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -3 }}
                  transition={{ duration: 0.16, ease: "easeOut" }}
                  className={clsx(
                    "flex min-h-0 flex-1 flex-col",
                    active === "dashboard" ? "overflow-hidden" : "overflow-y-auto"
                  )}
                >
                  {active === "dashboard" && (
                    <Dashboard
                      onOpenSkills={(agentId) => inspectAgent(agentId ?? "")}
                      onOpenChanges={(batch) => navigate("changes", { batch })}
                      onOpenSettings={() => navigate("settings")}
                    />
                  )}
                  {active !== "dashboard" && (
                    <div className="mx-auto w-full max-w-[1400px] px-4 py-3">
                      {active === "skills" && (
                        <Skills
                          search={search}
                          onSearch={setSearch}
                          initialAgent={skillsAgent}
                          onOpenDiff={setDiffSkill}
                          onOpenTerminal={(cwd, name) =>
                            openTerminal(cwd, { title: name ?? t("app.skillDirTitle") })
                          }
                        />
                      )}
                      {active === "mcp" && <Mcp />}
                      {active === "instructions" && <Instructions />}
                      {active === "projects" && (
                        <Projects
                          focusRoot={projectsFocus}
                          onOpenTerminal={(root, name) =>
                            openTerminal(root, { title: name ?? t("app.projectTerminal") })
                          }
                        />
                      )}
                      {active === "changes" && <Changes focusBatch={route.batch} />}
                      {active === "agents" && (
                        <Agents focusKey={agentsFocus} onInspectAgent={inspectAgent} />
                      )}
                      {active === "provider" && <Provider />}
                      {active === "settings" && <Settings />}
                      {active === "usage" && <Usage />}
                    </div>
                  )}
                </motion.div>
              </AnimatePresence>
            ))}
          </main>

          {inspectorOpen &&
            active !== "dashboard" &&
            active !== "terminal" &&
            active !== "usage" && (
            <Inspector
              onNavigate={(key) => navigate(key)}
              onInspectAgent={inspectAgent}
              onOpenDiff={setDiffSkill}
              onOpenAgentEnv={(agentId) => openAgentEnv(agentId)}
              onClose={() => setInspectorOpen(false)}
            />
          )}
        </div>

        <StatusBar
          onRollback={(batch) => void rollback(batch)}
          onOpenChanges={(batch) => navigate("changes", { batch })}
        />
      </div>

      <PlanDrawer />
      <CommandPalette commands={commands} />
      <ContextMenuHost />
      {diffSkill && <SkillDiffDialog name={diffSkill} onClose={() => setDiffSkill(null)} />}

      <AnimatePresence>
        {toast && (
          <motion.div
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: 6 }}
            className="fixed bottom-11 left-1/2 z-50 -translate-x-1/2 rounded-md border border-line bg-ink-900 px-3 py-1.5 text-[12px] text-text-strong shadow-panel"
          >
            {toast}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
    </MenuEnvProvider>
  );
}

export default function App() {
  return (
    // 语言在最外层：主题、数据、选中、面板的文案都可能带本地化标签
    <LocaleProvider>
      <ThemeProvider>
        <HubProvider>
          <SelectionProvider>
            <PaletteProvider>
              <ContextMenuProvider>
                <Console />
              </ContextMenuProvider>
            </PaletteProvider>
          </SelectionProvider>
        </HubProvider>
      </ThemeProvider>
    </LocaleProvider>
  );
}
