import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { clsx } from "clsx";
import { CornerDownLeft, Search } from "lucide-react";
import {
  GROUP_LABEL,
  GROUP_ORDER,
  SHORTCUTS,
  formatCombo,
  scoreCommand,
  type AppCommand,
  type CommandGroup,
} from "../../lib/commands";
import { usePalette } from "../../lib/palette";
import { useLocaleTag, useT } from "../../lib/i18n";

interface Row {
  command: AppCommand;
}

export function CommandPalette({ commands }: { commands: AppCommand[] }) {
  const { open, mode, close, recent, remember } = usePalette();
  const t = useT();
  const localeTag = useLocaleTag();
  const [query, setQuery] = useState("");
  const [activeIndex, setActiveIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    setQuery("");
    setActiveIndex(0);
    const timer = window.setTimeout(() => inputRef.current?.focus(), 20);
    return () => window.clearTimeout(timer);
  }, [open, mode]);

  const rows = useMemo<Row[]>(() => {
    const ranked = commands
      .map((command) => ({ command, score: scoreCommand(query, command) }))
      .filter((entry) => entry.score > 0);

    const recentRank = (id: string) => {
      const index = recent.indexOf(id);
      return index < 0 ? Number.MAX_SAFE_INTEGER : index;
    };

    ranked.sort((a, b) => {
      if (query.trim()) {
        if (b.score !== a.score) return b.score - a.score;
      } else {
        const byRecent = recentRank(a.command.id) - recentRank(b.command.id);
        if (byRecent !== 0) return byRecent;
      }
      const byGroup =
        GROUP_ORDER.indexOf(a.command.group) - GROUP_ORDER.indexOf(b.command.group);
      if (byGroup !== 0) return byGroup;
      // 没有查询时保持注册顺序（跳转项就是 NAV 的顺序，也就是 1..6）
      if (!query.trim()) return 0;
      return a.command.label.localeCompare(b.command.label, localeTag);
    });

    return ranked.map((entry) => ({ command: entry.command }));
  }, [commands, query, recent, localeTag]);

  const flat = rows;

  useEffect(() => {
    if (activeIndex > flat.length - 1) setActiveIndex(Math.max(0, flat.length - 1));
  }, [flat.length, activeIndex]);

  useEffect(() => {
    const node = listRef.current?.querySelector<HTMLElement>(`[data-index="${activeIndex}"]`);
    node?.scrollIntoView({ block: "nearest" });
  }, [activeIndex]);

  const runAt = useCallback(
    (index: number) => {
      const row = flat[index];
      if (!row) return;
      const command = row.command;
      if (command.enabled === false) return;
      remember(command.id);
      close();
      // 等浮层卸载后再执行，避免动作触发重渲染时面板还在挂载
      window.setTimeout(() => void command.run(), 0);
    },
    [flat, remember, close]
  );

  const onKeyDown = (event: React.KeyboardEvent) => {
    if (event.key === "Escape") {
      event.preventDefault();
      close();
      return;
    }
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setActiveIndex((index) => Math.min(index + 1, Math.max(0, flat.length - 1)));
      return;
    }
    if (event.key === "ArrowUp") {
      event.preventDefault();
      setActiveIndex((index) => Math.max(0, index - 1));
      return;
    }
    if (event.key === "Home") {
      event.preventDefault();
      setActiveIndex(0);
      return;
    }
    if (event.key === "End") {
      event.preventDefault();
      setActiveIndex(Math.max(0, flat.length - 1));
      return;
    }
    if (event.key === "Enter") {
      event.preventDefault();
      runAt(activeIndex);
    }
  };

  const grouped = useMemo(() => {
    const map = new Map<CommandGroup, { command: AppCommand; index: number }[]>();
    flat.forEach((row, index) => {
      const list = map.get(row.command.group) ?? [];
      list.push({ command: row.command, index });
      map.set(row.command.group, list);
    });
    return GROUP_ORDER.filter((group) => map.has(group)).map((group) => ({
      group,
      items: map.get(group) ?? [],
    }));
  }, [flat]);

  const renderOption = (command: AppCommand, index: number) => {
    const disabled = command.enabled === false;
    return (
      <button
        key={command.id}
        type="button"
        id={`palette-item-${index}`}
        data-index={index}
        role="option"
        aria-selected={index === activeIndex}
        aria-disabled={disabled}
        onMouseMove={() => setActiveIndex(index)}
        onClick={() => runAt(index)}
        className={clsx(
          "relative flex w-full items-center gap-2 px-3 py-2 text-left transition-colors",
          disabled ? "cursor-not-allowed" : "cursor-pointer",
          index === activeIndex && !disabled && "bg-surface-hover"
        )}
      >
        {index === activeIndex && !disabled && (
          <span className="absolute inset-y-1 left-0 w-0.5 rounded-full bg-brand-500" />
        )}
        <span className="min-w-0 flex-1">
          <span
            className={clsx(
              "block truncate text-[12.5px]",
              disabled ? "text-text-dim" : "text-text-strong"
            )}
          >
            {command.label}
          </span>
          {(command.reason ?? command.hint) && (
            <span className="mt-0.5 block truncate text-[11.5px] text-text-dim">
              {command.reason ?? command.hint}
            </span>
          )}
        </span>
        {command.shortcut && (
          <span className="mono shrink-0 rounded border border-line px-1.5 py-0.5 text-[11px] text-text-dim">
            {command.shortcut}
          </span>
        )}
      </button>
    );
  };

  return (
    <AnimatePresence>
      {open && (
        <motion.div
          key="palette-overlay"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.12 }}
          className="fixed inset-0 z-[70] flex items-start justify-center bg-[rgb(var(--scrim)/0.22)] px-6 pt-[12vh] dark:bg-[rgb(var(--scrim)/0.55)]"
          onMouseDown={(event) => {
            if (event.target === event.currentTarget) close();
          }}
        >
          <motion.div
            role="dialog"
            aria-modal="true"
            aria-label={
              mode === "commands" ? t("palette.title.commands") : t("palette.title.shortcuts")
            }
            data-palette
            initial={{ opacity: 0, y: -6 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -4 }}
            transition={{ duration: 0.16, ease: "easeOut" }}
            className="flex max-h-[60vh] w-full flex-col overflow-hidden rounded-lg border border-line bg-ink-900 shadow-[var(--shadow-drawer)] md:w-[560px]"
            onKeyDown={onKeyDown}
          >
            {mode === "commands" ? (
              <>
                <div className="flex h-11 shrink-0 items-center gap-2 border-b border-line px-3">
                  <Search className="h-3.5 w-3.5 shrink-0 text-text-dim" />
                  <input
                    ref={inputRef}
                    value={query}
                    onChange={(event) => {
                      setQuery(event.target.value);
                      setActiveIndex(0);
                    }}
                    role="combobox"
                    aria-expanded="true"
                    aria-controls="palette-list"
                    aria-autocomplete="list"
                    aria-activedescendant={
                      flat[activeIndex] ? `palette-item-${activeIndex}` : undefined
                    }
                    placeholder={t("palette.placeholder")}
                    data-palette-input
                    className="h-full w-full border-0 bg-transparent p-0 text-[12.5px] text-text-strong outline-none placeholder:text-text-dim focus:ring-0"
                  />
                  <span className="mono shrink-0 rounded border border-line px-1.5 py-0.5 text-[11px] text-text-dim">
                    {formatCombo("esc")}
                  </span>
                </div>

                <div
                  ref={listRef}
                  id="palette-list"
                  role="listbox"
                  aria-label={t("palette.commandsLabel")}
                  className="min-h-0 flex-1 overflow-y-auto py-1"
                >
                  {flat.length === 0 && (
                    <p className="px-3 py-6 text-center text-[11.5px] text-text-dim">
                      {t("palette.empty")}
                    </p>
                  )}

                  {query.trim()
                    ? // 有查询时按分数全局排序：最好的匹配必须落在第一行，不能被分组顺序打散
                      flat.map(({ command }, index) => renderOption(command, index))
                    : grouped.map(({ group, items }) => (
                        <div key={group}>
                          <p className="px-3 pb-1 pt-2 text-[11px] uppercase tracking-[0.08em] text-text-dim">
                            {t(GROUP_LABEL[group])}
                          </p>
                          {items.map(({ command, index }) => renderOption(command, index))}
                        </div>
                      ))}
                </div>

                <div className="flex h-7 shrink-0 items-center gap-2 overflow-hidden border-t border-line px-3 text-[11px] text-text-dim">
                  <span data-palette-count className="shrink-0">
                    {t("palette.results", { count: flat.length })}
                  </span>
                  <span className="shrink-0">·</span>
                  <span className="shrink-0">{t("palette.selectHint")}</span>
                  <span className="flex shrink-0 items-center gap-1">
                    <CornerDownLeft className="h-3 w-3" />
                    {t("palette.run")}
                  </span>
                  <span className="ml-auto hidden shrink-0 md:inline">
                    {t("palette.shortcutCount", { count: SHORTCUTS.length })}
                  </span>
                </div>
              </>
            ) : (
              <div className="min-h-0 flex-1 overflow-y-auto px-4 py-3" data-shortcuts>
                <div className="mb-2 flex items-center justify-between">
                  <h2 className="text-[13px] font-semibold text-text-strong">
                    {t("palette.title.shortcuts")}
                  </h2>
                  <button
                    type="button"
                    onClick={close}
                    className="cursor-pointer rounded border border-line px-1.5 py-0.5 text-[11px] text-text-dim transition-colors hover:text-text-strong"
                  >
                    {t("palette.close")}
                  </button>
                </div>
                <ul className="grid grid-cols-1 gap-x-6 gap-y-1.5 sm:grid-cols-2">
                  {SHORTCUTS.map((item) => (
                    <li
                      key={item.combo}
                      className="flex items-center justify-between gap-3 border-b border-line/60 py-1.5"
                    >
                      <span className="text-[11.5px] text-text-muted">{t(item.label)}</span>
                      <span className="mono shrink-0 rounded border border-line bg-surface-2 px-1.5 py-0.5 text-[11px] text-text-muted">
                        {formatCombo(item.combo)}
                      </span>
                    </li>
                  ))}
                </ul>
                <p className="mt-3 text-[11.5px] text-text-dim">
                  {t("palette.shortcutsNote")}
                </p>
              </div>
            )}
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
