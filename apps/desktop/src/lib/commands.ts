import type { MessageKey } from "./i18n/messages";
import { tr } from "./i18n/runtime";
import { navShortcut, visibleNav, type NavKey } from "./nav";
import type { FeatureFlags } from "./types";

export type CommandGroup = "jump" | "action" | "search";

export interface AppCommand {
  id: string;
  group: CommandGroup;
  label: string;
  /** 分组内的第二行说明 */
  hint?: string;
  keywords?: string[];
  /** 展示用快捷键（由 formatCombo 生成） */
  shortcut?: string;
  /** false 时仍然可见但置灰，用 reason 说明原因 */
  enabled?: boolean;
  reason?: string;
  run: () => void | Promise<void>;
}

/** 快捷键的唯一事实来源：面板、帮助页、文档都从这里取。 */
export const SHORTCUTS: { combo: string; label: MessageKey }[] = [
  { combo: "mod+k", label: "shortcut.palette" },
  { combo: "mod+1..9", label: "shortcut.modules" },
  { combo: "mod+0", label: "shortcut.usage" },
  { combo: "mod+shift+p", label: "shortcut.provider" },
  { combo: "mod+j", label: "shortcut.inspector" },
  { combo: "mod+b", label: "shortcut.nav" },
  { combo: "mod+shift+r", label: "shortcut.rescan" },
  { combo: "?", label: "shortcut.help" },
  { combo: "esc", label: "shortcut.esc" },
];

export const IS_MAC =
  typeof navigator !== "undefined" && /mac/i.test(navigator.platform || navigator.userAgent);

/** `mod+shift+r` → `⌘⇧R`（Mac）或 `Ctrl+Shift+R`（其它）。 */
export function formatCombo(combo: string): string {
  const parts = combo.split("+");
  const map: Record<string, string> = {
    mod: IS_MAC ? "⌘" : "Ctrl",
    shift: IS_MAC ? "⇧" : "Shift",
    alt: IS_MAC ? "⌥" : "Alt",
    esc: "Esc",
    enter: "Enter",
  };
  const rendered = parts.map((part) => map[part] ?? part.toUpperCase());
  return IS_MAC ? rendered.join("") : rendered.join("+");
}

/** 子序列模糊匹配；0 表示不匹配，分数越高越靠前。 */
export function fuzzyScore(query: string, target: string): number {
  const q = query.toLowerCase();
  const t = target.toLowerCase();
  if (!q) return 1;
  const direct = t.indexOf(q);
  if (direct >= 0) {
    return 200 + (direct === 0 ? 60 : 0) + (t.length === q.length ? 40 : 0) - t.length * 0.1;
  }
  let ti = 0;
  let consecutive = 0;
  let best = 0;
  let matched = 0;
  for (const ch of q) {
    const found = t.indexOf(ch, ti);
    if (found < 0) return 0;
    const isWordStart = found === 0 || /[\s/\-_.]/.test(t[found - 1] ?? "");
    if (found === ti) consecutive += 1;
    else consecutive = 0;
    best += 4 + consecutive * 6 + (isWordStart ? 8 : 0);
    matched += 1;
    ti = found + 1;
  }
  if (matched !== q.length) return 0;
  return best - t.length * 0.05;
}

/**
 * 命令匹配打分。label 权重最高，keywords 次之，hint 只作兜底——
 * 否则输入「去重」时，「转到技能库」的 hint（扫描 / 去重 / 接管）会压过「生成去重计划」。
 */
export function scoreCommand(query: string, cmd: AppCommand): number {
  if (!query.trim()) return 1;
  const label = fuzzyScore(query, cmd.label);
  const keyword = (cmd.keywords ?? []).reduce(
    (best, word) => Math.max(best, fuzzyScore(query, word)),
    0
  );
  const hint = cmd.hint ? fuzzyScore(query, cmd.hint) : 0;
  return Math.max(label, keyword * 0.9, hint * 0.4);
}

export interface CommandContext {
  navigate: (page: NavKey) => void;
  /** 功能开关：关闭的模块不进「转到」列表，也不该在面板里搜到 */
  features: FeatureFlags;
  /** 网关是否在跑：决定「启动 / 停止网关」哪一条可用 */
  gatewayRunning: boolean;
  startGateway: () => void;
  stopGateway: () => void;
  refresh: () => void;
  planDedupe: () => void;
  planRepair: () => void;
  applyPlan: () => void;
  rollbackLatest: () => void;
  toggleTheme: () => void;
  toggleNav: () => void;
  toggleInspector: () => void;
  openShortcuts: () => void;
  hasPlan: boolean;
  planOps: number;
  applying: boolean;
  latestBatch: { id: string; undone: boolean } | null;
  skills: string[];
  projects: { name: string; root: string }[];
  agents: { id: string; name: string }[];
  /** 常见 Agent 目录里的产品（含未接入的），用于「打开 Agent 环境」 */
  catalog: { key: string; name: string; hint: string }[];
  openSkill: (name: string) => void;
  openAgent: (id: string) => void;
  openAgentEnv: (key: string) => void;
  /** 当前选中的技能名（来自 selection），没有选中时为 null */
  skillDiffName: string | null;
  openSkillDiff: (name: string) => void;
  /** 在工作目录 cwd 打开一个新终端标签 */
  openTerminal: (
    cwd: string,
    extra?: { title?: string; command?: string | null; agentId?: string | null }
  ) => void;
}

/**
 * 组装命令表：跳转复用 NAV，动作复用 useHub，搜索项由数据动态生成。
 *
 * 文案走 `tr()`（模块级当前语言）：调用方把它放进带语言依赖的 `useMemo`，
 * 切换语言时整张表重建，标签与置灰理由都会跟着变。
 */
export function buildCommands(ctx: CommandContext): AppCommand[] {
  const jumps: AppCommand[] = visibleNav(ctx.features).map((item) => {
    const combo = navShortcut(item.key);
    const label = tr(item.label);
    return {
      id: `jump:${item.key}`,
      group: "jump" as const,
      label: tr("cmd.jumpTo", { label }),
      hint: tr(item.hint),
      // 关键词保留中英双语：英文界面下用中文词也能搜到同一条命令
      keywords: [item.key, label],
      shortcut: combo ? formatCombo(combo) : undefined,
      run: () => ctx.navigate(item.key),
    };
  });

  const actions: AppCommand[] = [
    {
      id: "action:refresh",
      group: "action",
      label: tr("cmd.refresh"),
      hint: tr("cmd.refresh.hint"),
      keywords: ["scan", "reload", "扫描"],
      shortcut: formatCombo("mod+shift+r"),
      run: ctx.refresh,
    },
    {
      id: "action:plan-dedupe",
      group: "action",
      label: tr("cmd.planDedupe"),
      hint: tr("cmd.planDedupe.hint"),
      keywords: ["dedupe", "duplicate", "去重"],
      run: ctx.planDedupe,
    },
    {
      id: "action:plan-repair",
      group: "action",
      label: tr("cmd.planRepair"),
      hint: tr("cmd.planRepair.hint"),
      keywords: ["repair", "fix", "broken", "修复"],
      run: ctx.planRepair,
    },
    {
      id: "action:apply",
      group: "action",
      label: tr("cmd.applyPlan"),
      hint: ctx.hasPlan
        ? tr("cmd.applyPlan.hint", { count: ctx.planOps })
        : tr("cmd.applyPlan.none"),
      keywords: ["apply", "execute", "执行"],
      enabled: ctx.hasPlan && !ctx.applying,
      reason: ctx.applying
        ? tr("cmd.applyPlan.running")
        : ctx.hasPlan
          ? undefined
          : tr("cmd.applyPlan.needPlan"),
      run: ctx.applyPlan,
    },
    {
      id: "action:rollback",
      group: "action",
      label: tr("cmd.rollback"),
      hint: ctx.latestBatch
        ? ctx.latestBatch.undone
          ? tr("cmd.rollback.undone", { id: ctx.latestBatch.id })
          : ctx.latestBatch.id
        : tr("cmd.rollback.none"),
      keywords: ["rollback", "undo", "回滚"],
      enabled: Boolean(ctx.latestBatch && !ctx.latestBatch.undone),
      reason: ctx.latestBatch?.undone ? tr("cmd.rollback.reason") : undefined,
      run: ctx.rollbackLatest,
    },
    {
      id: "action:gateway-start",
      group: "action",
      label: tr("cmd.gatewayStart"),
      hint: ctx.gatewayRunning ? tr("cmd.gatewayStart.running") : tr("cmd.gatewayStart.hint"),
      keywords: ["gateway", "proxy", "网关", "代理"],
      enabled: ctx.features.provider && !ctx.gatewayRunning,
      reason: !ctx.features.provider
        ? tr("cmd.gatewayStart.disabled")
        : ctx.gatewayRunning
          ? tr("cmd.gatewayStart.already")
          : undefined,
      run: ctx.startGateway,
    },
    {
      id: "action:gateway-stop",
      group: "action",
      label: tr("cmd.gatewayStop"),
      hint: tr("cmd.gatewayStop.hint"),
      keywords: ["gateway", "stop", "网关", "停止"],
      enabled: ctx.features.provider && ctx.gatewayRunning,
      reason: ctx.gatewayRunning ? undefined : tr("cmd.gatewayStop.reason"),
      run: ctx.stopGateway,
    },
    {
      id: "action:theme",
      group: "action",
      label: tr("cmd.theme"),
      hint: tr("cmd.theme.hint"),
      keywords: ["theme", "dark", "light", "主题", "暗色", "亮色"],
      run: ctx.toggleTheme,
    },
    {
      id: "action:nav",
      group: "action",
      label: tr("cmd.nav"),
      keywords: ["sidebar", "nav", "导航"],
      shortcut: formatCombo("mod+b"),
      run: ctx.toggleNav,
    },
    {
      id: "action:inspector",
      group: "action",
      label: tr("cmd.inspector"),
      keywords: ["inspector", "panel", "检查器"],
      shortcut: formatCombo("mod+j"),
      run: ctx.toggleInspector,
    },
    {
      id: "action:skill-diff",
      group: "action",
      label: tr("cmd.diff"),
      hint: ctx.skillDiffName
        ? tr("cmd.diff.hint", { name: ctx.skillDiffName })
        : tr("cmd.diff.needSelect"),
      keywords: ["diff", "compare", "差异", "对比", "版本"],
      enabled: Boolean(ctx.skillDiffName),
      reason: ctx.skillDiffName ? undefined : tr("cmd.diff.reason"),
      run: () => {
        if (ctx.skillDiffName) ctx.openSkillDiff(ctx.skillDiffName);
      },
    },
    {
      id: "action:shortcuts",
      group: "action",
      label: tr("cmd.shortcuts"),
      keywords: ["keyboard", "help", "快捷键", "帮助"],
      shortcut: "?",
      run: ctx.openShortcuts,
    },
  ];

  const skillItems: AppCommand[] = ctx.skills.map((name) => ({
    id: `skill:${name}`,
    group: "search",
    label: name,
    hint: tr("cmd.hintSkill"),
    keywords: ["skill", "技能"],
    run: () => ctx.openSkill(name),
  }));

  const projectItems: AppCommand[] = ctx.projects.map((project) => ({
    id: `project:${project.root}`,
    group: "search",
    label: project.name,
    hint: tr("cmd.hintProject"),
    keywords: ["project", "项目", project.root],
    run: () => ctx.navigate("projects"),
  }));

  /** 「在终端打开某个项目」：只给最近三个，命令面板不是项目管理器 */
  const terminalItems: AppCommand[] = ctx.projects.slice(0, 3).map((project) => ({
    id: `terminal:${project.root}`,
    group: "action",
    label: tr("cmd.openInTerminal", { name: project.name }),
    hint: project.root,
    keywords: ["terminal", "终端", "shell", "cwd", project.root],
    run: () => ctx.openTerminal(project.root, { title: project.name }),
  }));

  const catalogItems: AppCommand[] = ctx.catalog.map((item) => ({
    id: `catalog:${item.key}`,
    group: "search",
    label: item.name,
    hint: tr("cmd.hintCatalog", { hint: item.hint }),
    keywords: ["agent", "install", "安装", "检测", item.key],
    run: () => ctx.openAgentEnv(item.key),
  }));

  const agentItems: AppCommand[] = ctx.agents.map((agent) => ({
    id: `agent:${agent.id}`,
    group: "search",
    label: agent.name,
    hint: tr("cmd.hintAgent"),
    keywords: ["agent", agent.id],
    run: () => ctx.openAgent(agent.id),
  }));

  return [
    ...jumps,
    ...actions,
    ...terminalItems,
    ...catalogItems,
    ...agentItems,
    ...skillItems,
    ...projectItems,
  ];
}

export const GROUP_LABEL: Record<CommandGroup, MessageKey> = {
  jump: "cmd.group.jump",
  action: "cmd.group.action",
  search: "cmd.group.search",
};

export const GROUP_ORDER: CommandGroup[] = ["jump", "action", "search"];
