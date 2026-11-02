import type { ITheme } from "@xterm/xterm";
import { DARK_THEMES } from "./dark";
import { LIGHT_THEMES } from "./light";
import type { AppTheme, ThemeKind } from "./types";

export type { AppTheme, ThemeKind, ThemeSwatch } from "./types";

/** 两套默认都指向 GitHub（亮/暗各一套），老用户与首次启动都落在这里 */
export const DEFAULT_THEME_ID: Record<ThemeKind, string> = {
  dark: "github-dark",
  light: "github-light",
};

export const THEMES: Record<ThemeKind, AppTheme[]> = {
  dark: DARK_THEMES,
  light: LIGHT_THEMES,
};

export function themesOf(kind: ThemeKind): AppTheme[] {
  return THEMES[kind];
}

/**
 * 按 id 取主题；id 非法（旧存储、手改 localStorage、跨模式 id）一律回落默认，
 * 保证任何脏数据都不会让界面停在「没有调色板」的状态。
 */
export function themeById(kind: ThemeKind, id: string | null | undefined): AppTheme {
  const list = THEMES[kind];
  const found = id ? list.find((theme) => theme.id === id) : undefined;
  if (found) return found;
  return list.find((theme) => theme.id === DEFAULT_THEME_ID[kind]) ?? list[0];
}

export function isThemeId(kind: ThemeKind, id: string | null | undefined): boolean {
  return Boolean(id && THEMES[kind].some((theme) => theme.id === id));
}

/** xterm 只吃具体色值：把主题里的终端色板交出去（保持调用方不关心数据结构） */
export function terminalTheme(theme: AppTheme): ITheme {
  return theme.terminal;
}

/** 顶栏提示、设置页「生效中」等处共用的展示名 */
export function themeLabel(theme: AppTheme): string {
  return theme.name;
}
