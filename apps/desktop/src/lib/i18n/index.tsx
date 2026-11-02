import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api } from "../api";
import { detectSystemLocale, readStoredMode, resolveMode, storeMode } from "./detect";
import { translate } from "./format";
import type { TFn } from "./messages";
import { currentLocale, setActiveLocale } from "./runtime";
import { bcp47, isLocaleMode, type Locale, type LocaleMode } from "./types";

interface LocaleContextValue {
  /** 用户选择，含「跟随系统」 */
  mode: LocaleMode;
  /** 实际生效的语言 */
  resolved: Locale;
  /**
   * 每次语言发生变化 +1。
   *
   * 后端返回的文案（变更计划、安装步骤、错误）是「取值那一刻的快照」，
   * 把组件里存了这些快照的 effect 带上它，切换语言后才会重新取一遍。
   */
  revision: number;
  setMode: (mode: LocaleMode) => void;
  /** 用后端 `config.json` 里的权威值回填，不写回 localStorage 之外的东西 */
  adoptMode: (mode: LocaleMode) => void;
}

const LocaleContext = createContext<LocaleContextValue | null>(null);

/**
 * 原生窗口是否可用。
 *
 * 这里直接读 `__TAURI_INTERNALS__` 而不是 import `lib/api`：api 会连带
 * `demo.ts`（几千行假数据）进来，而语言 provider 在最外层，不该为它拉这么大一坨。
 */
function hasTauriWindow(): boolean {
  return Boolean((window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__);
}

/** 把语言落到 `<html lang>` 与两处标题上。 */
function applyDocumentMeta(locale: Locale): void {
  document.documentElement.lang = bcp47(locale);
  document.title = translate(locale, "app.title");
  if (!hasTauriWindow()) return;
  getCurrentWindow()
    .setTitle(translate(locale, "app.windowTitle"))
    .catch((err) => console.warn("同步窗口标题失败", err));
}

export function LocaleProvider({ children }: { children: ReactNode }) {
  const [mode, setModeState] = useState<LocaleMode>(readStoredMode);
  const [resolved, setResolved] = useState<Locale>(() => resolveMode(mode));
  const [revision, setRevision] = useState(0);
  /** 给 `adoptMode` 用：它不订阅 `mode`，否则每次挂载同步都重建回调、effect 反复触发 */
  const modeRef = useRef(mode);
  modeRef.current = mode;

  // 渲染期就把模块级语言对齐：`tr()` 在同一帧里被纯函数调用时不会读到上一门语言
  useMemo(() => setActiveLocale(resolved), [resolved]);

  // 跟随系统时，系统语言在运行期间变了要重解析（macOS 改语言不必重启应用）
  useEffect(() => {
    if (mode !== "system" || typeof window === "undefined") return;
    const onChange = () => setResolved(detectSystemLocale());
    window.addEventListener("languagechange", onChange);
    return () => window.removeEventListener("languagechange", onChange);
  }, [mode]);

  useEffect(() => {
    setActiveLocale(resolved);
    applyDocumentMeta(resolved);
  }, [resolved]);

  /**
   * 启动时与后端对齐：`config.json` 里的 `locale` 是权威值（CLI 也可能改过它），
   * localStorage 只是首帧镜像。后端还没起来 / 演示模式时静默保持本地检测结果。
   */
  useEffect(() => {
    let cancelled = false;
    void api
      .locale()
      .then((view) => {
        if (cancelled || !isLocaleMode(view.mode)) return;
        if (view.mode === modeRef.current) return;
        storeMode(view.mode);
        setModeState(view.mode);
        setResolved(resolveMode(view.mode));
        setRevision((value) => value + 1);
      })
      .catch(() => {
        // 读不到就用本地判断，不该因为一次 IPC 失败把界面卡在英文
      });
    return () => {
      cancelled = true;
    };
  }, []);

  /** 用户在设置页选的：本地立即生效，同时把权威值写回 `config.json`。 */
  const setMode = useCallback((next: LocaleMode) => {
    storeMode(next);
    setModeState(next);
    const effective = resolveMode(next);
    setResolved(effective);
    setRevision((value) => value + 1);
    void api.setLocale(next, effective).catch((err) => {
      // 写不进去只影响下次启动的语言，本次会话照常
      console.warn("保存界面语言失败", err);
    });
  }, []);

  /** 不改配置，只把后端/系统给的值落到界面上。 */
  const adoptMode = useCallback((next: LocaleMode) => {
    if (next === modeRef.current) return;
    storeMode(next);
    setModeState(next);
    setResolved(resolveMode(next));
    setRevision((value) => value + 1);
  }, []);

  const value = useMemo<LocaleContextValue>(
    () => ({ mode, resolved, revision, setMode, adoptMode }),
    [mode, resolved, revision, setMode, adoptMode]
  );

  return <LocaleContext.Provider value={value}>{children}</LocaleContext.Provider>;
}

export function useLocale(): LocaleContextValue {
  const ctx = useContext(LocaleContext);
  if (!ctx) throw new Error("useLocale 必须在 LocaleProvider 内使用");
  return ctx;
}

/**
 * 组件里的取词函数。
 *
 * 依赖 `resolved`：切换语言时所有调用它的组件都会重渲染，
 * 不会出现「一半界面已经换了、另一半还是旧语言」。
 */
export function useT(): TFn {
  const { resolved } = useLocale();
  return useCallback((key, args) => translate(resolved, key, args), [resolved]);
}

/** 排序、日期、数字格式化用的标签（`zh-CN` / `zh-TW` / `en-US`）。 */
export function useLocaleTag(): string {
  const { resolved } = useLocale();
  return bcp47(resolved);
}

/** 给非组件代码用的只读取词入口（不订阅变化）。 */
export { tr as translateNow, currentLocale } from "./runtime";
export type { TFn } from "./messages";
export type { Locale, LocaleMode } from "./types";
export { LOCALE_MODES, LOCALE_SELF_NAME, isLocaleMode } from "./types";
export { LOCALE_STORAGE_KEY } from "./detect";
