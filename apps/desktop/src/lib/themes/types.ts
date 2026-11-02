import type { ITheme } from "@xterm/xterm";

/** 主题只分两类：由界面模式（亮/暗/跟随系统）决定拿哪一类 */
export type ThemeKind = "dark" | "light";

/** 设置页里做预览的 4 个色点（值同时用于取强调色的 hex，不参与 CSS 令牌） */
export interface ThemeSwatch {
  bg: string;
  panel: string;
  accent: string;
  text: string;
}

export interface AppTheme {
  id: string;
  /** 展示名，取自各主题官方发布名 */
  name: string;
  /** 来源标注：设置页卡片右上角的小字 */
  family: string;
  kind: ThemeKind;
  swatch: ThemeSwatch;
  /** xterm 配色：背景/前景/光标/选区 + ANSI 16 色 */
  terminal: ITheme;
}
