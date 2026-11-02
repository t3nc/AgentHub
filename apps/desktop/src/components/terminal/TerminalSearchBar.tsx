import { useEffect, useRef, useState } from "react";
import { clsx } from "clsx";
import { ChevronDown, ChevronUp, Search, X } from "lucide-react";
import { useT } from "../../lib/i18n";

export interface TerminalSearchBarProps {
  open: boolean;
  onClose: () => void;
  /** 返回是否命中；backwards = 向上找 */
  onSearch: (query: string, backwards: boolean) => boolean;
  /** 关闭时清掉高亮 */
  onClear: () => void;
  subscribe: (listener: (index: number, count: number) => void) => () => void;
}

/**
 * 终端内搜索条：贴在标签栏右侧，36px 高，和标签同一行。
 *
 * 只做「输入 + 上下跳 + 计数」，高亮由 xterm 的 SearchAddon 负责；
 * 组件本身不持有 addon，避免和 TerminalPane 抢生命周期。
 */
export function TerminalSearchBar({
  open,
  onClose,
  onSearch,
  onClear,
  subscribe,
}: TerminalSearchBarProps) {
  const t = useT();
  const inputRef = useRef<HTMLInputElement | null>(null);
  const [query, setQuery] = useState("");
  const [result, setResult] = useState({ index: -1, count: 0 });

  useEffect(() => {
    if (!open) return;
    const unsubscribe = subscribe((index, count) => setResult({ index, count }));
    // 等一帧再聚焦：刚展开时容器还没布局，select() 会滚到错位置
    const frame = window.requestAnimationFrame(() => {
      inputRef.current?.focus();
      inputRef.current?.select();
    });
    return () => {
      unsubscribe();
      window.cancelAnimationFrame(frame);
    };
  }, [open, subscribe]);

  // 关闭时把状态清干净，下次打开不残留上次的计数
  useEffect(() => {
    if (open) return;
    setQuery("");
    setResult({ index: -1, count: 0 });
    onClear();
  }, [open, onClear]);

  if (!open) return null;

  const hasQuery = query.length > 0;
  const empty = hasQuery && result.count === 0;

  return (
    <div className="flex h-6 shrink-0 items-center gap-1 rounded-md border border-line bg-ink-950/70 pl-1.5 pr-0.5">
      <Search className="h-3 w-3 shrink-0 text-text-dim" />
      <input
        ref={inputRef}
        value={query}
        spellCheck={false}
        placeholder={t("terminal.searchAria")}
        aria-label={t("search.aria")}
        onChange={(event) => {
          const next = event.target.value;
          setQuery(next);
          if (next) onSearch(next, false);
          else onClear();
        }}
        onKeyDown={(event) => {
          if (event.key === "Escape") {
            event.preventDefault();
            onClose();
            return;
          }
          if (event.key === "Enter") {
            event.preventDefault();
            if (query) onSearch(query, event.shiftKey);
          }
        }}
        className="h-full w-[150px] min-w-0 bg-transparent text-[11px] text-text-strong outline-none placeholder:text-text-dim/70"
      />
      <span
        className={clsx(
          "shrink-0 font-mono text-[11px] tabular-nums",
          empty ? "text-state-danger" : "text-text-dim"
        )}
      >
        {hasQuery ? (empty ? "0/0" : `${result.index + 1}/${result.count}`) : ""}
      </span>
      <button
        type="button"
        aria-label={t("search.prevMatch")}
        title={t("search.prev")}
        disabled={!hasQuery}
        onClick={() => onSearch(query, true)}
        className="shrink-0 cursor-pointer rounded p-0.5 text-text-dim transition-colors hover:text-text-strong disabled:cursor-default disabled:opacity-40"
      >
        <ChevronUp className="h-3 w-3" />
      </button>
      <button
        type="button"
        aria-label={t("search.nextMatch")}
        title={t("search.next")}
        disabled={!hasQuery}
        onClick={() => onSearch(query, false)}
        className="shrink-0 cursor-pointer rounded p-0.5 text-text-dim transition-colors hover:text-text-strong disabled:cursor-default disabled:opacity-40"
      >
        <ChevronDown className="h-3 w-3" />
      </button>
      <button
        type="button"
        aria-label={t("terminal.close")}
        title={t("search.close")}
        onClick={onClose}
        className="shrink-0 cursor-pointer rounded p-0.5 text-text-dim transition-colors hover:text-state-danger"
      >
        <X className="h-3 w-3" />
      </button>
    </div>
  );
}
