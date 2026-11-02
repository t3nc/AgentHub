import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type MouseEvent as ReactMouseEvent,
  type ReactNode,
} from "react";
import type { AppCommand } from "./commands";
import type { NavKey } from "./nav";
import { isEditable } from "./hotkeys";
import type { MenuEntry } from "./menu";

/** 打开中的菜单：坐标是视口 CSS 像素，与 `clientX/clientY` 同一坐标系。 */
export interface MenuOpenState {
  key: number;
  x: number;
  y: number;
  entries: MenuEntry[];
  /** 关闭后要把焦点还给谁（键盘触发时是焦点元素，鼠标触发时是 body） */
  anchor: HTMLElement | null;
}

interface ContextMenuValue {
  state: MenuOpenState | null;
  open: (event: ReactMouseEvent, entries: MenuEntry[]) => void;
  openAt: (x: number, y: number, entries: MenuEntry[], anchor?: HTMLElement | null) => void;
  close: () => void;
}

const ContextMenuContext = createContext<ContextMenuValue | null>(null);

/**
 * 是否该把右键还给系统原生菜单。
 *
 * 决策：输入类控件、内嵌终端正文、以及页面上当前存在选中文本时一律放行，
 * 否则用户会丢掉「复制 / 粘贴」。这个判断必须在 `preventDefault()` **之前**做。
 */
function keepNative(target: EventTarget | null): boolean {
  // 输入框 / 下拉 / contenteditable：复制粘贴必须留在系统菜单里
  if (isEditable(target)) return true;
  // 内嵌终端正文（xterm 自己管选区与剪贴板）
  if (target instanceof Element && target.closest(".xterm")) return true;
  // 页面上已经有选中文本：用户多半是想复制，别抢
  const selection = typeof window !== "undefined" ? window.getSelection() : null;
  if (selection && !selection.isCollapsed && selection.toString().trim().length > 0) {
    return true;
  }
  return false;
}

export function ContextMenuProvider({ children }: { children: ReactNode }) {
  const [state, setState] = useState<MenuOpenState | null>(null);
  const seq = useRef(0);

  const close = useCallback(() => {
    setState((prev) => {
      if (prev?.anchor && prev.anchor.isConnected) {
        // 等菜单卸载后再还焦点，否则会被菜单自己的 focus 抢走
        window.setTimeout(() => prev.anchor?.focus?.(), 0);
      }
      return null;
    });
  }, []);

  const openAt = useCallback(
    (x: number, y: number, entries: MenuEntry[], anchor: HTMLElement | null = null) => {
      if (entries.length === 0) return;
      seq.current += 1;
      setState({ key: seq.current, x, y, entries, anchor });
    },
    []
  );

  const open = useCallback(
    (event: ReactMouseEvent, entries: MenuEntry[]) => {
      if (keepNative(event.target)) return;
      event.preventDefault();
      event.stopPropagation();
      const anchor =
        event.currentTarget instanceof HTMLElement ? event.currentTarget : null;
      openAt(event.clientX, event.clientY, entries, anchor);
    },
    [openAt]
  );

  /** `Shift+F10` / 菜单键：把事件转发给焦点元素，复用同一套行级菜单 */
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const isMenuKey =
        event.key === "ContextMenu" || (event.key === "F10" && event.shiftKey);
      if (!isMenuKey) return;
      event.preventDefault();
      const target =
        document.activeElement instanceof HTMLElement ? document.activeElement : null;
      const rect = target?.getBoundingClientRect();
      const x = rect ? Math.min(rect.left + 24, window.innerWidth - 24) : window.innerWidth / 2;
      const y = rect ? Math.min(rect.bottom - 8, window.innerHeight - 24) : window.innerHeight / 2;
      if (!target || target === document.body) {
        // 没有聚焦元素：交给页面根节点上的菜单（它会自己弹页面级菜单）
        document.body.dispatchEvent(
          new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: x, clientY: y })
        );
        return;
      }
      target.dispatchEvent(
        new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: x, clientY: y })
      );
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  /** 页面切换（hash 变化）时菜单必须收起，否则会停在新页面上空 */
  useEffect(() => {
    const onHash = () => setState(null);
    window.addEventListener("hashchange", onHash);
    return () => window.removeEventListener("hashchange", onHash);
  }, []);

  const value = useMemo<ContextMenuValue>(
    () => ({ state, open, openAt, close }),
    [state, open, openAt, close]
  );

  return (
    <ContextMenuContext.Provider value={value}>{children}</ContextMenuContext.Provider>
  );
}

export function useContextMenu(): ContextMenuValue {
  const ctx = useContext(ContextMenuContext);
  if (!ctx) throw new Error("useContextMenu 必须在 ContextMenuProvider 内使用");
  return ctx;
}

/**
 * 页面构建菜单时需要的 App 级动作。
 * 与 `CommandContext`（命令面板）分开：那边是「全局命令」，这边是「页面能把行级动作接上」。
 */
export interface MenuEnv {
  commands: AppCommand[];
  /** 打开命令面板（它不是 AppCommand，单独给一个入口） */
  openCommands: () => void;
  navigate: (page: NavKey) => void;
  openTerminal: (cwd: string, extra?: { title?: string }) => void;
  openSkill: (name: string) => void;
  openAgent: (agentId: string) => void;
  openAgentEnv: (key?: string | null) => void;
  openSkillDiff: (name: string) => void;
  skillDiffName: string | null;
}

const MenuEnvContext = createContext<MenuEnv | null>(null);

export function MenuEnvProvider({ value, children }: { value: MenuEnv; children: ReactNode }) {
  return <MenuEnvContext.Provider value={value}>{children}</MenuEnvContext.Provider>;
}

export function useMenuEnv(): MenuEnv {
  const ctx = useContext(MenuEnvContext);
  if (!ctx) throw new Error("useMenuEnv 必须在 MenuEnvProvider 内使用");
  return ctx;
}
