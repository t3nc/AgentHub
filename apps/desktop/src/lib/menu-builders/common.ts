import {
  ArrowRightLeft,
  Command as CommandIcon,
  Eye,
  FolderOpen,
  GitCompare,
  Keyboard,
  Layers,
  PanelLeft,
  PanelRight,
  Play,
  RefreshCw,
  RotateCcw,
  Square,
  SquareTerminal,
  SunMoon,
  Wrench,
  type LucideIcon,
} from "lucide-react";
import { formatCombo } from "../commands";
import { tr } from "../i18n/runtime";
import type { MenuEnv } from "../context-menu";
import { fromCommand, isMenuItem, sep, tidy, type MenuEntry, type MenuItem } from "../menu";
import type { NavKey } from "../nav";
import { copyText } from "../clipboard";
import { openWithDefault, revealPath } from "../open";

type Notify = (message: string) => void;

/** 命令 id → 图标。没列到的走无图标渲染，不影响可用性。 */
const ICONS: Record<string, LucideIcon> = {
  "action:refresh": RefreshCw,
  "action:plan-dedupe": Layers,
  "action:plan-repair": Wrench,
  "action:apply": Play,
  "action:rollback": RotateCcw,
  "action:gateway-start": Play,
  "action:gateway-stop": Square,
  "action:theme": SunMoon,
  "action:nav": PanelLeft,
  "action:inspector": PanelRight,
  "action:skill-diff": GitCompare,
  "action:shortcuts": Keyboard,
};

/** 通用组里希望固定在前面的顺序；其余命令原样跟在后面。 */
const ACTION_ORDER = [
  "action:refresh",
  "action:gateway-start",
  "action:gateway-stop",
  "action:plan-dedupe",
  "action:plan-repair",
  "action:apply",
  "action:rollback",
  "action:skill-diff",
  "action:theme",
  "action:nav",
  "action:inspector",
  "action:shortcuts",
];

/** 慢动作：执行期间菜单保留并显示转圈 */
const PENDING = new Set(["action:refresh", "action:rollback", "action:apply", "action:plan-dedupe"]);

function paletteItem(env: MenuEnv): MenuItem {
  return {
    id: "menu:palette",
    label: tr("menu.palette"),
    hint: tr("menu.palette.hint"),
    icon: CommandIcon,
    shortcut: formatCombo("mod+k"),
    run: env.openCommands,
  };
}

function globalSection(env: MenuEnv): MenuEntry[] {
  // `terminal:<项目根>` 是命令面板里的「在终端打开某项目」，它属于搜索面，不进上下文菜单
  const actions = env.commands.filter(
    (command) => command.group === "action" && !command.id.startsWith("terminal:")
  );
  const ordered = [
    ...ACTION_ORDER.map((id) => actions.find((command) => command.id === id)).filter(
      (command) => command !== undefined
    ),
    ...actions.filter((command) => !ACTION_ORDER.includes(command.id)),
  ];
  return [paletteItem(env), sep(), ...ordered.map((command) =>
    fromCommand(command, { icon: ICONS[command.id], pending: PENDING.has(command.id) })
  )];
}

/** 「转到 …」子菜单：列出其它模块，当前页不出现 */
function jumpSection(env: MenuEnv, current: NavKey): MenuEntry[] {
  const children = env.commands
    .filter((command) => command.group === "jump" && command.id !== `jump:${current}`)
    .map((command) => fromCommand(command, { id: `goto:${command.id}` }));
  if (children.length === 0) return [];
  return [
    {
      id: "menu:goto",
      label: tr("menu.goto"),
      icon: ArrowRightLeft,
      children,
    },
  ];
}

/**
 * 统一收尾：条目级/页面级 + 通用组 + 跳转子菜单。
 * 所有页面都走这一个入口，菜单结构才不会各页各样。
 */
export function composeMenu(
  entries: MenuEntry[],
  env: MenuEnv,
  current: NavKey
): MenuEntry[] {
  return tidy([...entries, sep(), ...globalSection(env), sep(), ...jumpSection(env, current)]);
}

/** 危险项统一放在分组末尾，并自带一条分隔线 */
export function dangerSection(entries: MenuEntry[]): MenuEntry[] {
  return entries.length > 0 ? [sep(), ...entries] : [];
}

export function copyItem(
  label: string,
  value: string,
  notify: Notify,
  options: { id?: string; icon?: LucideIcon } = {}
): MenuItem {
  return {
    id: options.id ?? `copy:${label}:${value}`,
    label,
    icon: options.icon,
    // 复制成功的提示不再回显 label：文案是本地化的，拼「已复制+label」在英文下会串味
    run: () => copyText(value).then((ok) => notify(ok ? tr("menu.copied") : tr("menu.copyFailed"))),
  };
}

export function revealItem(path: string, notify: Notify, id = "reveal"): MenuItem {
  return {
    id,
    label: tr("menu.reveal"),
    icon: FolderOpen,
    run: () => revealPath(path).then((result) => notify(result.message)),
  };
}

export function openFileItem(
  path: string,
  notify: Notify,
  label = tr("menu.openWith")
): MenuItem {
  return {
    id: `open:${path}`,
    label,
    icon: Eye,
    run: () => openWithDefault(path).then((result) => notify(result.message)),
  };
}

export function terminalItem(
  path: string,
  env: MenuEnv,
  label = tr("menu.terminalHere")
): MenuItem {
  return {
    id: `terminal:${path}`,
    label,
    icon: SquareTerminal,
    run: () => env.openTerminal(path),
  };
}

/** 菜单里只放「可读」的条目：分隔线与纯文本标签不进键盘导航 */
export function countActionable(entries: MenuEntry[]): number {
  return entries.filter((entry) => isMenuItem(entry) && !entry.disabled).length;
}
