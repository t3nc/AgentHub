import { useState } from "react";
import { clsx } from "clsx";
import { ArrowDown, ArrowUp } from "lucide-react";
import { compactTokens, money, thousands } from "../../lib/format";
import { useT } from "../../lib/i18n";
import type { MessageKey } from "../../lib/i18n/messages";
import type { UsageGroup } from "../../lib/types";
import type { UsageSortKey, SortDirection } from "../../lib/usage";

const COLUMNS: {
  key: UsageSortKey;
  /** `provider` 是专有名词，不翻译，所以这一列没有 key。 */
  label?: MessageKey;
  numeric: boolean;
  title?: MessageKey;
}[] = [
  { key: "model", label: "umodel.col.model", numeric: false, title: "umodel.col.modelTitle" },
  { key: "provider", numeric: false, title: "umodel.col.providerTitle" },
  { key: "input", label: "umodel.col.input", numeric: true, title: "umodel.col.inputTitle" },
  {
    key: "cache_read",
    label: "umodel.col.cacheRead",
    numeric: true,
    title: "umodel.col.cacheReadTitle",
  },
  { key: "output", label: "umodel.col.output", numeric: true, title: "umodel.col.outputTitle" },
  { key: "calls", label: "umodel.col.calls", numeric: true, title: "umodel.col.callsTitle" },
  { key: "cost", label: "umodel.col.cost", numeric: true, title: "umodel.col.costTitle" },
];

const COLLAPSED_ROWS = 12;

/**
 * 模型明细表：可排序，数值右对齐等宽。
 *
 * 排序状态由页面持有（切范围 / 筛选后要保持），这里只发事件。
 */
export function ModelTable({
  rows,
  providerOf,
  sortKey,
  sortDir,
  onSort,
  onPick,
}: {
  rows: UsageGroup[];
  providerOf: (group: UsageGroup) => string;
  sortKey: UsageSortKey;
  sortDir: SortDirection;
  onSort: (key: UsageSortKey) => void;
  onPick?: (model: string) => void;
}) {
  const t = useT();
  const [expanded, setExpanded] = useState(false);
  const visible = expanded ? rows : rows.slice(0, COLLAPSED_ROWS);

  if (rows.length === 0) {
    return (
      <p className="py-6 text-center text-[12px] text-text-dim">{t("umodel.empty")}</p>
    );
  }

  return (
    <div>
      <div className="overflow-x-auto">
        <table className="w-full min-w-[720px] border-collapse text-[12px]">
          <thead>
            <tr className="border-b border-line text-left">
              {COLUMNS.map((column) => {
                const active = sortKey === column.key;
                return (
                  <th
                    key={column.key}
                    data-usage-col={column.key}
                    title={column.title ? t(column.title) : undefined}
                    className={clsx(
                      "px-2 py-2 font-medium text-text-dim",
                      column.numeric ? "text-right" : "text-left"
                    )}
                  >
                    <button
                      type="button"
                      onClick={() => onSort(column.key)}
                      className={clsx(
                        "inline-flex cursor-pointer items-center gap-1 transition-colors hover:text-text-strong",
                        active && "text-text-strong"
                      )}
                    >
                      {column.label ? t(column.label) : "provider"}
                      {active &&
                        (sortDir === "asc" ? (
                          <ArrowUp className="h-3 w-3" />
                        ) : (
                          <ArrowDown className="h-3 w-3" />
                        ))}
                    </button>
                  </th>
                );
              })}
            </tr>
          </thead>
          <tbody>
            {visible.map((row) => {
              const unpriced = row.cost === null || row.cost === undefined;
              return (
                <tr
                  key={row.key}
                  onClick={() => onPick?.(row.key)}
                  className={clsx(
                    "border-b border-line/60 transition-colors last:border-0",
                    onPick && "cursor-pointer hover:bg-surface-hover"
                  )}
                  data-usage-model={row.key}
                >
                  <td className="max-w-[240px] truncate px-2 py-2 text-text-strong" title={row.key}>
                    {row.key}
                  </td>
                  <td className="px-2 py-2 text-text-dim">{providerOf(row)}</td>
                  <td className="px-2 py-2 text-right font-mono tabular-nums" title={thousands(row.input)}>
                    {compactTokens(row.input)}
                  </td>
                  <td className="px-2 py-2 text-right font-mono tabular-nums" title={thousands(row.cache_read)}>
                    {compactTokens(row.cache_read)}
                  </td>
                  <td className="px-2 py-2 text-right font-mono tabular-nums" title={thousands(row.output)}>
                    {compactTokens(row.output)}
                  </td>
                  <td className="px-2 py-2 text-right font-mono tabular-nums text-text-dim">
                    {thousands(row.calls)}
                  </td>
                  <td
                    className={clsx(
                      "px-2 py-2 text-right font-mono tabular-nums",
                      unpriced ? "text-text-dim" : "text-text-strong"
                    )}
                  >
                    {unpriced ? t("umodel.unpriced") : money(row.cost ?? 0, row.currency)}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>

      {rows.length > COLLAPSED_ROWS && (
        <div className="mt-2 flex justify-center">
          <button
            type="button"
            onClick={() => setExpanded((prev) => !prev)}
            className="cursor-pointer rounded-md px-2 py-1 text-[11.5px] text-text-dim transition-colors hover:bg-surface-hover hover:text-text-strong"
          >
            {expanded
              ? t("umodel.collapse")
              : t("umodel.expand", { count: rows.length })}
          </button>
        </div>
      )}
    </div>
  );
}
