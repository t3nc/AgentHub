import {
  Bot,
  Boxes,
  FileText,
  FolderGit2,
  Gauge,
  History,
  LayoutDashboard,
  Network,
  Plug,
  SlidersHorizontal,
  SquareTerminal,
} from "lucide-react";
import type { MessageKey } from "./i18n/messages";
import { tr } from "./i18n/runtime";
import type { FeatureFlags, FeatureKey } from "./types";

export type NavKey =
  | "dashboard"
  | "terminal"
  | "skills"
  | "mcp"
  | "instructions"
  | "projects"
  | "changes"
  | "agents"
  | "provider"
  | "settings"
  | "usage";

export interface NavItem {
  key: NavKey;
  /** 文案 key：真正显示的文字由 `t()` 解析，切换语言时跟着变 */
  label: MessageKey;
  hint: MessageKey;
  icon: typeof Boxes;
  /**
   * 快捷键 combo（唯一真源）。
   *
   * 不再按下标算 `mod+{index+1}`：新增模块时那套算法会把所有既有编号顶掉，
   * 也会在模块超过 9 个后产生 `mod+10` 这种非法组合。
   */
  shortcut?: string;
  /** 归属的功能开关；不填 = 永远显示。功能关闭时入口从导航 / 命令面板 / 右键菜单消失。 */
  feature?: FeatureKey;
}

/**
 * 模块定义：导航与工具条共用同一份，避免标题和导航文案漂移。
 *
 * 编号固定写死在这里：`mod+1..9` 是既有肌肉记忆，`usage` 仍是 `mod+0`，
 * 新增的 Provider 用 `mod+shift+P` —— 一个都不改。
 */
export const NAV: NavItem[] = [
  {
    key: "dashboard",
    label: "nav.dashboard",
    hint: "nav.dashboard.hint",
    icon: LayoutDashboard,
    shortcut: "mod+1",
  },
  {
    key: "terminal",
    label: "nav.terminal",
    hint: "nav.terminal.hint",
    icon: SquareTerminal,
    shortcut: "mod+2",
  },
  {
    key: "skills",
    label: "nav.skills",
    hint: "nav.skills.hint",
    icon: Boxes,
    shortcut: "mod+3",
  },
  {
    key: "mcp",
    label: "nav.mcp",
    hint: "nav.mcp.hint",
    icon: Plug,
    shortcut: "mod+4",
  },
  {
    key: "instructions",
    label: "nav.instructions",
    hint: "nav.instructions.hint",
    icon: FileText,
    shortcut: "mod+5",
  },
  {
    key: "projects",
    label: "nav.projects",
    hint: "nav.projects.hint",
    icon: FolderGit2,
    shortcut: "mod+6",
  },
  {
    key: "changes",
    label: "nav.changes",
    hint: "nav.changes.hint",
    icon: History,
    shortcut: "mod+7",
  },
  {
    key: "agents",
    label: "nav.agents",
    hint: "nav.agents.hint",
    icon: Bot,
    shortcut: "mod+8",
  },
  {
    key: "provider",
    label: "nav.provider",
    hint: "nav.provider.hint",
    icon: Network,
    shortcut: "mod+shift+p",
    feature: "provider",
  },
  {
    key: "settings",
    label: "nav.settings",
    hint: "nav.settings.hint",
    icon: SlidersHorizontal,
    shortcut: "mod+9",
  },
  {
    key: "usage",
    label: "nav.usage",
    hint: "nav.usage.hint",
    icon: Gauge,
    shortcut: "mod+0",
    feature: "usage",
  },
];

export const VALID_NAV: NavKey[] = NAV.map((item) => item.key);

export function navItem(key: NavKey): NavItem {
  return NAV.find((item) => item.key === key) ?? NAV[0];
}

/** 模块名（按当前界面语言）。 */
export function navLabel(key: NavKey): string {
  return tr(navItem(key).label);
}

/** 模块说明（按当前界面语言）。 */
export function navHint(key: NavKey): string {
  return tr(navItem(key).hint);
}

/**
 * 当前可见的模块。
 *
 * 只影响渲染与命令表，不影响快捷键映射 —— 否则禁用一项后其余编号会整体前移，
 * 用户按 `mod+3` 打开的可能已经不是技能库了。
 */
export function visibleNav(features?: FeatureFlags | null): NavItem[] {
  if (!features) return NAV;
  return NAV.filter((item) => !item.feature || features[item.feature]);
}

/** 该模块归属的功能开关；没有归属返回 null。 */
export function featureOf(key: NavKey): FeatureKey | null {
  return navItem(key).feature ?? null;
}

/** 模块的快捷键组合；没分配返回 null。 */
export function navShortcut(key: NavKey): string | null {
  return navItem(key).shortcut ?? null;
}
