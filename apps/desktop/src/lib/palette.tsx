import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";

export type PaletteMode = "commands" | "shortcuts";

interface PaletteContextValue {
  open: boolean;
  mode: PaletteMode;
  openCommands: () => void;
  openShortcuts: () => void;
  /** 已打开时再按一次 mod+k 视为关闭 */
  toggleCommands: () => void;
  close: () => void;
  recent: string[];
  remember: (id: string) => void;
}

const RECENT_KEY = "agenthub-recent-commands";
const RECENT_MAX = 5;

const PaletteContext = createContext<PaletteContextValue | null>(null);

function readRecent(): string[] {
  try {
    const raw = window.localStorage.getItem(RECENT_KEY);
    if (!raw) return [];
    const parsed: unknown = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed.filter((item): item is string => typeof item === "string") : [];
  } catch {
    // 存储被禁用 / 内容损坏：当作没有历史
    return [];
  }
}

function writeRecent(ids: string[]) {
  try {
    window.localStorage.setItem(RECENT_KEY, JSON.stringify(ids));
  } catch {
    // 忽略：没记住只影响下次排序
  }
}

export function PaletteProvider({ children }: { children: ReactNode }) {
  const [open, setOpen] = useState(false);
  const [mode, setMode] = useState<PaletteMode>("commands");
  const [recent, setRecent] = useState<string[]>(readRecent);
  // 打开前的焦点元素：关闭后要还回去，否则键盘用户会「掉焦点」
  const triggerRef = useRef<HTMLElement | null>(null);

  const rememberTrigger = useCallback(() => {
    const active = document.activeElement;
    triggerRef.current = active instanceof HTMLElement ? active : null;
  }, []);

  const openCommands = useCallback(() => {
    rememberTrigger();
    setMode("commands");
    setOpen(true);
  }, [rememberTrigger]);

  const openShortcuts = useCallback(() => {
    rememberTrigger();
    setMode("shortcuts");
    setOpen(true);
  }, [rememberTrigger]);

  const close = useCallback(() => {
    setOpen(false);
    const trigger = triggerRef.current;
    triggerRef.current = null;
    // 等浮层卸载后再还焦点，避免被 autofocus 抢走
    window.setTimeout(() => trigger?.focus?.(), 0);
  }, []);

  const toggleCommands = useCallback(() => {
    if (open && mode === "commands") close();
    else openCommands();
  }, [open, mode, close, openCommands]);

  const remember = useCallback((id: string) => {
    setRecent((prev) => {
      const next = [id, ...prev.filter((item) => item !== id)].slice(0, RECENT_MAX);
      writeRecent(next);
      return next;
    });
  }, []);

  const value = useMemo<PaletteContextValue>(
    () => ({ open, mode, openCommands, openShortcuts, toggleCommands, close, recent, remember }),
    [open, mode, openCommands, openShortcuts, toggleCommands, close, recent, remember]
  );

  return <PaletteContext.Provider value={value}>{children}</PaletteContext.Provider>;
}

export function usePalette(): PaletteContextValue {
  const ctx = useContext(PaletteContext);
  if (!ctx) throw new Error("usePalette 必须在 PaletteProvider 内使用");
  return ctx;
}
