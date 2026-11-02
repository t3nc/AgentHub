import { clsx } from "clsx";
import { compactTokens, percent } from "../../lib/format";
import type { UsageGroup } from "../../lib/types";

/**
 * 横向占比条：agent 或 provider 的消耗分布。
 *
 * 点击即筛选（与上方芯片双向联动）。`active` 为空表示「全部」，此时所有条目都是常态。
 */
export function ProviderBreakdown({
  groups,
  total,
  active,
  onToggle,
  emptyHint,
  iconOf,
}: {
  groups: UsageGroup[];
  total: number;
  active: string[];
  onToggle: (key: string) => void;
  emptyHint: string;
  /** agent 用品牌图标；provider 没有图标时传 undefined */
  iconOf?: (key: string) => React.ReactNode;
}) {
  if (groups.length === 0) {
    return <p className="py-3 text-[12px] text-text-dim">{emptyHint}</p>;
  }

  return (
    <ul className="flex flex-col gap-1.5">
      {groups.map((group) => {
        const selected = active.includes(group.key);
        const dimmed = active.length > 0 && !selected;
        const width = total > 0 ? Math.max((group.total / total) * 100, 1.5) : 0;
        return (
          <li key={group.key}>
            <button
              type="button"
              onClick={() => onToggle(group.key)}
              className={clsx(
                "group w-full cursor-pointer rounded-md px-1.5 py-1.5 text-left transition-colors",
                selected ? "bg-surface-active" : "hover:bg-surface-hover",
                dimmed && "opacity-55"
              )}
              data-breakdown={group.key}
              aria-pressed={selected}
            >
              <div className="flex items-center gap-1.5">
                {iconOf?.(group.key)}
                <span className="min-w-0 flex-1 truncate text-[12px] text-text-strong" title={group.label}>
                  {group.label}
                </span>
                <span className="shrink-0 font-mono text-[11.5px] text-text-muted tabular-nums">
                  {compactTokens(group.total)}
                </span>
                <span className="w-10 shrink-0 text-right text-[11px] text-text-dim tabular-nums">
                  {percent(group.total, total)}
                </span>
              </div>
              <div className="mt-1 h-1 overflow-hidden rounded-full bg-surface-2">
                <div
                  className={clsx(
                    "h-full rounded-full transition-[width] duration-200 ease-out",
                    selected ? "bg-brand-400" : "bg-brand-500/55 group-hover:bg-brand-500/80"
                  )}
                  style={{ width: `${width}%` }}
                />
              </div>
            </button>
          </li>
        );
      })}
    </ul>
  );
}
