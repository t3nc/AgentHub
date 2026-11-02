import { clsx } from "clsx";
import { compactTokens, money, percent } from "../../lib/format";
import { useT } from "../../lib/i18n";
import type { MessageKey } from "../../lib/i18n/messages";
import type { UsageTotals } from "../../lib/types";

interface MetricItem {
  key: "input" | "cache_read" | "output" | "total";
  label: MessageKey;
  /** 语义色变量名（`--ok` / `--info` …），图例与进度条共用 */
  tone: string;
  hint: MessageKey;
}

const ITEMS: MetricItem[] = [
  {
    key: "input",
    label: "umodel.col.input",
    tone: "--info",
    hint: "metric.inputHint",
  },
  {
    key: "cache_read",
    label: "umodel.col.cacheRead",
    tone: "--ok",
    hint: "metric.cacheReadHint",
  },
  { key: "output", label: "umodel.col.output", tone: "--warn", hint: "metric.outputHint" },
  { key: "total", label: "trend.total", tone: "--brand-500", hint: "metric.totalHint" },
];

/**
 * 指标条：三项计量 + 合计 + 估算花费。
 *
 * 数值是「非缓存输入」而不是「总输入」——两种日志口径在解析层已经归一到这个语义，
 * 这里不再做任何加减，避免出现「界面上再算一次」的双份逻辑。
 *
 * 「新建缓存」不展示：本机各来源实测恒为 0，留着只会让人以为漏统计。
 * 它仍计入 `totals.total`，只是不再单列。
 */
export function UsageMetrics({ totals }: { totals: UsageTotals }) {
  const t = useT();
  const empty = totals.total === 0;
  return (
    <div className="grid grid-cols-2 gap-2 lg:grid-cols-3 xl:grid-cols-5" data-usage-metrics>
      {ITEMS.map((item) => {
        const value = totals[item.key];
        const ratio = totals.total > 0 ? (value / totals.total) * 100 : 0;
        return (
          <div
            key={item.key}
            className={clsx(
              "panel px-3.5 py-3 transition-colors",
              item.key === "total" && "border-brand-500/30"
            )}
            title={t(item.hint)}
            data-metric={item.key}
            data-value={value}
          >
            <div className="flex items-baseline justify-between gap-2">
              <span className="text-[11.5px] text-text-dim">{t(item.label)}</span>
              <span className="text-[11px] text-text-dim tabular-nums">
                {item.key === "total" ? "100%" : percent(value, totals.total)}
              </span>
            </div>
            <div className="mt-1 font-mono text-[20px] leading-none text-text-strong tabular-nums">
              {compactTokens(value)}
            </div>
            <div className="mt-2.5 h-1 overflow-hidden rounded-full bg-surface-2">
              <div
                className="h-full rounded-full transition-[width] duration-200 ease-out"
                style={{
                  width: empty ? "0%" : `${Math.max(ratio, value > 0 ? 1.5 : 0)}%`,
                  backgroundColor: `rgb(var(${item.tone}))`,
                }}
              />
            </div>
          </div>
        );
      })}

      <div
        className="panel px-3.5 py-3"
        data-metric="cost"
        data-value={totals.cost === null || totals.cost === undefined ? "" : totals.cost.toFixed(4)}
        data-unpriced={totals.unpriced_calls}
      >
        <div className="flex items-baseline justify-between gap-2">
          <span className="text-[11.5px] text-text-dim">{t("umodel.col.cost")}</span>
          <span className="text-[11px] text-text-dim">{totals.currency}</span>
        </div>
        <div className="mt-1 font-mono text-[20px] leading-none text-text-strong tabular-nums">
          {totals.cost === null || totals.cost === undefined
            ? "—"
            : money(totals.cost, "").trim()}
        </div>
        <div className="mt-2.5 text-[11px] text-text-dim">
          {totals.cost === null || totals.cost === undefined
            ? t("metric.allUnpriced")
            : totals.unpriced_calls > 0
              ? t("metric.someUnpriced", { count: compactTokens(totals.unpriced_calls) })
              : t("metric.allPriced")}
        </div>
      </div>
    </div>
  );
}
