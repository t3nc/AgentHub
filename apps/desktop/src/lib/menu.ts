import type { LucideIcon } from "lucide-react";
import type { AppCommand } from "./commands";

/**
 * 右键菜单的数据模型。
 *
 * 刻意与 `AppCommand`（命令面板）保持同构：label / hint / shortcut / enabled / reason 一一对应，
 * 这样「重新扫描」这类全局动作在两处的文案、快捷键、置灰理由只有一份事实来源，
 * 用 `fromCommand` 适配即可，不在菜单里另抄一遍。
 */
export interface MenuItem {
  kind?: "item";
  id: string;
  label: string;
  /** 第二行说明（条目级菜单用，例如技能路径） */
  hint?: string;
  icon?: LucideIcon;
  /** 右侧快捷键（如 Ctrl+Shift+R），与 hint 二选一 */
  shortcut?: string;
  /** 危险操作：赤红文字，固定排在所在分组的最后 */
  danger?: boolean;
  disabled?: boolean;
  /** 置灰原因，悬停时显示在右侧 */
  reason?: string;
  /**
   * 慢动作（重新扫描、检测版本）：执行期间保留菜单并显示转圈。
   * 默认 false —— 菜单项执行前先关菜单，避免浮层压住随后弹出的对话框。
   */
  pending?: boolean;
  /** 子菜单（如「转到」） */
  children?: MenuEntry[];
  run?: () => void | Promise<void>;
}

export type MenuEntry = MenuItem | { kind: "separator" } | { kind: "label"; text: string };

export const SEP: MenuEntry = { kind: "separator" };

export function sep(): MenuEntry {
  return SEP;
}

export function groupLabel(text: string): MenuEntry {
  return { kind: "label", text };
}

export function isMenuItem(entry: MenuEntry): entry is MenuItem {
  return entry.kind !== "separator" && entry.kind !== "label";
}

export function isSeparator(entry: MenuEntry): boolean {
  return entry.kind === "separator";
}

/** `AppCommand` → 菜单项。`enabled === false` 映射成置灰 + 原因。 */
export function fromCommand(command: AppCommand, overrides: Partial<MenuItem> = {}): MenuItem {
  return {
    id: `cmd:${command.id}`,
    label: command.label,
    hint: command.hint,
    shortcut: command.shortcut,
    disabled: command.enabled === false,
    reason: command.reason,
    run: () => command.run(),
    ...overrides,
  };
}

/**
 * 收尾：去掉首尾与连续的重复分隔线。
 * 页面按需拼「条目级 / 页面级 / 通用 / 跳转」，拼完交给它兜底，避免出现两条挨着的横线。
 */
export function tidy(entries: MenuEntry[]): MenuEntry[] {
  const out: MenuEntry[] = [];
  for (const entry of entries) {
    if (isSeparator(entry) && (out.length === 0 || isSeparator(out[out.length - 1]))) continue;
    out.push(entry);
  }
  while (out.length > 0 && isSeparator(out[out.length - 1])) out.pop();
  return out;
}
