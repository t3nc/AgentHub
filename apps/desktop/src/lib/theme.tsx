import { Laptop, Moon, Sun } from "lucide-react";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useT } from "./i18n";
import type { MessageKey } from "./i18n/messages";
import { isTauri } from "./api";
import {
  DEFAULT_THEME_ID,
  isThemeId,
  themeById,
  type AppTheme,
  type ThemeKind,
} from "./themes";

export type ThemeMode = "light" | "dark" | "system";
export type ResolvedTheme = ThemeKind;

/** 模式与两套主题 id 分开存：切模式不该丢掉另一侧挑好的配色 */
const MODE_KEY = "agenthub-theme";
const DARK_KEY = "agenthub-theme-dark";
const LIGHT_KEY = "agenthub-theme-light";

function systemPrefersDark(): boolean {
  return (
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-color-scheme: dark)").matches
  );
}

export function resolveThemeKind(mode: ThemeMode): ThemeKind {
  return mode === "dark" || (mode === "system" && systemPrefersDark()) ? "dark" : "light";
}

export function readStoredMode(): ThemeMode {
  const raw = window.localStorage.getItem(MODE_KEY);
  return raw === "light" || raw === "dark" || raw === "system" ? raw : "system";
}

/** 两套主题的选择（非法值在 themeById 里统一回落默认） */
function readStoredThemeIds(): Record<ThemeKind, string> {
  return {
    dark: window.localStorage.getItem(DARK_KEY) || DEFAULT_THEME_ID.dark,
    light: window.localStorage.getItem(LIGHT_KEY) || DEFAULT_THEME_ID.light,
  };
}

/**
 * 把主题落到 <html> 上：`.dark` 驱动 Tailwind dark: 变体与原生控件，
 * `data-theme` 指向 index.css 里对应的调色板块 —— 两者必须同时更新。
 */
export function applyTheme(theme: AppTheme): void {
  const root = document.documentElement;
  root.classList.toggle("dark", theme.kind === "dark");
  root.dataset.theme = theme.id;
}

/**
 * 原生窗口装饰（标题栏、边框）不读 CSS 变量，只能走系统 API。
 *
 * - `light` / `dark`：把窗口钉在对应明暗，和页面换肤一致；
 * - `system`：传 null 交回系统，而不是钉成当前解析值 —— 否则系统主题再变时，
 *   窗口要等前端 matchMedia 回调纠正，多一层时序、还会和 webview 的
 *   prefers-color-scheme 互相覆盖。
 * 失败只告警：缺权限或平台不支持时，页面换肤照常。
 */
function syncWindowTheme(mode: ThemeMode, kind: ThemeKind): void {
  if (!isTauri) return;
  getCurrentWindow()
    .setTheme(mode === "system" ? null : kind)
    .catch((err) => console.warn("同步窗口标题栏主题失败", err));
}

interface ThemeContextValue {
  mode: ThemeMode;
  /** 当前解析出的明暗（follow system 时会随系统变） */
  resolved: ResolvedTheme;
  /** 当前生效的主题，含终端色板 */
  active: AppTheme;
  darkThemeId: string;
  lightThemeId: string;
  setMode: (mode: ThemeMode) => void;
  setDarkTheme: (id: string) => void;
  setLightTheme: (id: string) => void;
  resetThemes: () => void;
  cycle: () => void;
}

const ThemeContext = createContext<ThemeContextValue | null>(null);

const CYCLE: ThemeMode[] = ["system", "light", "dark"];

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [mode, setModeState] = useState<ThemeMode>(readStoredMode);
  const [ids, setIds] = useState<Record<ThemeKind, string>>(readStoredThemeIds);
  const [resolved, setResolved] = useState<ResolvedTheme>(() => resolveThemeKind(mode));

  const active = useMemo(() => themeById(resolved, ids[resolved]), [resolved, ids]);

  useEffect(() => {
    applyTheme(active);
  }, [active]);

  // 只跟明暗走，不跟整套主题走：同明暗的两套主题之间切换不必重复下发 IPC
  useEffect(() => {
    syncWindowTheme(mode, active.kind);
  }, [mode, active.kind]);

  useEffect(() => {
    if (mode !== "system" || typeof window.matchMedia !== "function") return;
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => setResolved(media.matches ? "dark" : "light");
    media.addEventListener("change", onChange);
    return () => media.removeEventListener("change", onChange);
  }, [mode]);

  const setMode = useCallback((next: ThemeMode) => {
    window.localStorage.setItem(MODE_KEY, next);
    setResolved(resolveThemeKind(next));
    setModeState(next);
  }, []);

  const setDarkTheme = useCallback((id: string) => {
    if (!isThemeId("dark", id)) return;
    window.localStorage.setItem(DARK_KEY, id);
    setIds((prev) => ({ ...prev, dark: id }));
  }, []);

  const setLightTheme = useCallback((id: string) => {
    if (!isThemeId("light", id)) return;
    window.localStorage.setItem(LIGHT_KEY, id);
    setIds((prev) => ({ ...prev, light: id }));
  }, []);

  /** 只重置配色，不动界面模式：用户可能只是想把主题退回 GitHub */
  const resetThemes = useCallback(() => {
    window.localStorage.removeItem(DARK_KEY);
    window.localStorage.removeItem(LIGHT_KEY);
    setIds({ ...DEFAULT_THEME_ID });
  }, []);

  const cycle = useCallback(() => {
    setMode(CYCLE[(CYCLE.indexOf(mode) + 1) % CYCLE.length]);
  }, [mode, setMode]);

  const value = useMemo<ThemeContextValue>(
    () => ({
      mode,
      resolved,
      active,
      darkThemeId: ids.dark,
      lightThemeId: ids.light,
      setMode,
      setDarkTheme,
      setLightTheme,
      resetThemes,
      cycle,
    }),
    [mode, resolved, active, ids, setMode, setDarkTheme, setLightTheme, resetThemes, cycle]
  );

  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
}

export function useTheme(): ThemeContextValue {
  const ctx = useContext(ThemeContext);
  if (!ctx) throw new Error("useTheme 必须在 ThemeProvider 内使用");
  return ctx;
}

const LABEL: Record<ThemeMode, MessageKey> = {
  system: "theme.system",
  light: "theme.light",
  dark: "theme.dark",
};

export function ThemeToggle() {
  const { mode, cycle, active } = useTheme();
  const t = useT();
  const Icon = mode === "light" ? Sun : mode === "dark" ? Moon : Laptop;
  const label = t(LABEL[mode]);
  return (
    <button
      type="button"
      onClick={cycle}
      title={t("theme.title", { label, name: active.name })}
      aria-label={t("theme.aria", { label, name: active.name })}
      className="flex cursor-pointer items-center gap-1.5 rounded-md border border-line bg-surface-2 px-2 py-1 text-[11.5px] text-text-muted transition-colors hover:border-line-strong hover:bg-surface-hover hover:text-text-strong"
    >
      <Icon className="h-3.5 w-3.5" />
      {LABEL[mode]}
    </button>
  );
}
