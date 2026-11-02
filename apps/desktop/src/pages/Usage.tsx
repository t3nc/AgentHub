import { useCallback, useEffect, useMemo, useState } from "react";
import { clsx } from "clsx";
import { RotateCw, Tags } from "lucide-react";
import { AgentIcon } from "../components/agents/AgentIcon";
import { ModelTable } from "../components/usage/ModelTable";
import { PricingDialog } from "../components/usage/PricingDialog";
import { ProviderBreakdown } from "../components/usage/ProviderBreakdown";
import { SourcePanel } from "../components/usage/SourcePanel";
import { UsageMetrics } from "../components/usage/UsageMetrics";
import { UsageTrend } from "../components/usage/UsageTrend";
import { Button, Empty, Panel } from "../components/ui";
import { useHub } from "../lib/useHub";
import { useT } from "../lib/i18n";
import { dayLabel } from "../lib/usage";
import { compactTokens } from "../lib/format";
import {
  EMPTY_FILTER,
  RANGE_OPTIONS,
  filterCells,
  groupCells,
  providerKey,
  seriesOf,
  sortGroups,
  totalsOf,
  type SortDirection,
  type UsageFilter,
  type UsageSortKey,
} from "../lib/usage";
import type { UsageRange } from "../lib/types";

/**
 * 用量页：只读观测台。
 *
 * 刻意**不进全局启动刷新**——首次统计要流式读几百 MB 会话日志，只有进入本页才拉。
 * 数据一次拿全（cells），范围与筛选都在客户端算，切筛选不走 IPC。
 */
export function Usage() {
  const { usage, usageLoading, refreshUsage, notify } = useHub();
  const t = useT();
  const [range, setRange] = useState<UsageRange>("days7");
  const [filter, setFilter] = useState<UsageFilter>(EMPTY_FILTER);
  const [sortKey, setSortKey] = useState<UsageSortKey>("input");
  const [sortDir, setSortDir] = useState<SortDirection>("desc");
  const [pricingOpen, setPricingOpen] = useState(false);

  useEffect(() => {
    void refreshUsage(range);
  }, [range, refreshUsage]);

  const toggleIn = useCallback((field: keyof UsageFilter, value: string) => {
    setFilter((prev) => {
      const list = prev[field];
      const next = list.includes(value)
        ? list.filter((item) => item !== value)
        : [...list, value];
      return { ...prev, [field]: next };
    });
  }, []);

  const agentLabel = useMemo(() => {
    const map = new Map<string, string>();
    for (const item of usage?.sources ?? []) {
      map.set(item.source.key, item.source.label);
    }
    return map;
  }, [usage]);

  /** 模型 → provider：日志只在 Codex 写了 provider，其余靠后端按价格表推断后落在 cell 上。 */
  const modelProvider = useMemo(() => {
    const map = new Map<string, string>();
    for (const cell of usage?.cells ?? []) {
      if (!map.has(cell.model)) map.set(cell.model, providerKey(cell.provider));
    }
    return map;
  }, [usage]);

  const view = useMemo(() => {
    if (!usage) return null;
    const cells = filterCells(usage.cells, filter);
    const prices = usage.prices;
    return {
      totals: totalsOf(cells, prices),
      series: seriesOf(cells),
      byAgent: groupCells(cells, (cell) => cell.agent, (key) => agentLabel.get(key) ?? key, prices),
      byProvider: groupCells(cells, (cell) => providerKey(cell.provider), (key) => key, prices),
      byModel: groupCells(cells, (cell) => cell.model, (key) => key, prices),
    };
  }, [usage, filter, agentLabel]);

  const sortedModels = useMemo(() => {
    if (!view) return [];
    return sortGroups(view.byModel, sortKey, sortDir, (group) => modelProvider.get(group.key) ?? "—");
  }, [view, sortKey, sortDir, modelProvider]);

  const onSort = (key: UsageSortKey) => {
    if (key === sortKey) setSortDir((prev) => (prev === "desc" ? "asc" : "desc"));
    else {
      setSortKey(key);
      setSortDir(key === "model" || key === "provider" ? "asc" : "desc");
    }
  };

  const hasFilter =
    filter.agents.length > 0 || filter.providers.length > 0 || filter.models.length > 0;
  const loadingFirst = usageLoading && !usage;
  const empty = usage && view && view.totals.total === 0;

  return (
    <div className="flex flex-col gap-3 pb-8">
      <header className="flex flex-wrap items-end justify-between gap-3">
        <div className="min-w-0">
          <p className="text-[11px] font-medium tracking-wide text-text-dim">
            {t("usage.kicker")}
          </p>
          <h1 className="mt-0.5 text-[20px] font-semibold leading-tight text-text-strong">
            {t("usage.title")}
          </h1>
          <p className="mt-1 text-[12px] leading-relaxed text-text-dim">
            {t("usage.subtitle")}
            {usage?.start
              ? t("usage.rangeFrom", { day: dayLabel(usage.start) })
              : t("usage.rangeAll")}
          </p>
        </div>

        <div className="flex flex-wrap items-center gap-2">
          <div className="flex items-center gap-0.5 rounded-md border border-line bg-surface-2 p-0.5">
            {RANGE_OPTIONS.map((option) => (
              <button
                key={option.key}
                type="button"
                title={t(option.hint)}
                onClick={() => setRange(option.key)}
                className={clsx(
                  "cursor-pointer rounded px-2.5 py-1 text-[11.5px] transition-colors",
                  range === option.key
                    ? "bg-brand-500/15 text-brand-300"
                    : "text-text-dim hover:text-text-strong"
                )}
                aria-pressed={range === option.key}
                data-range={option.key}
              >
                {t(option.label)}
              </button>
            ))}
          </div>
          <Button
            variant="outline"
            size="sm"
            disabled={usageLoading}
            onClick={() => void refreshUsage(range, true)}
            title={t("usage.rescanTitle")}
          >
            <RotateCw className={clsx("h-3.5 w-3.5", usageLoading && "animate-spin")} />
            {usageLoading ? t("usage.scanning") : t("usage.rescan")}
          </Button>
        </div>
      </header>

      <div className="flex flex-wrap items-center gap-1.5">
        <span className="text-[11px] text-text-dim">agent</span>
        {(usage?.by_agent ?? []).map((group) => {
          const active = filter.agents.includes(group.key);
          return (
            <button
              key={group.key}
              type="button"
              onClick={() => toggleIn("agents", group.key)}
              aria-pressed={active}
              className={clsx(
                "inline-flex cursor-pointer items-center gap-1.5 rounded-md border px-2 py-1 text-[11.5px] transition-colors",
                active
                  ? "border-brand-500/50 bg-brand-500/15 text-brand-300"
                  : "border-line text-text-muted hover:border-line hover:bg-surface-hover hover:text-text-strong"
              )}
              data-agent-chip={group.key}
            >
              <AgentIcon id={group.key} className="h-3.5 w-3.5" />
              {agentLabel.get(group.key) ?? group.label}
              <span className="font-mono tabular-nums text-text-dim">
                {compactTokens(group.total)}
              </span>
            </button>
          );
        })}

        {filter.models.map((model) => (
          <button
            key={model}
            type="button"
            onClick={() => toggleIn("models", model)}
            className="inline-flex cursor-pointer items-center gap-1.5 rounded-md border border-brand-500/50 bg-brand-500/15 px-2 py-1 text-[11.5px] text-brand-300"
            data-model-chip={model}
          >
            {t("usage.modelChip", { model })} ✕
          </button>
        ))}

        {hasFilter && (
          <Button variant="ghost" size="sm" onClick={() => setFilter(EMPTY_FILTER)}>
            {t("usage.clearFilter")}
          </Button>
        )}

        <span className="ml-auto text-[11px] text-text-dim">
          {usage
            ? t("usage.scanSummary", {
                files: usage.stats.files_total,
                cached: usage.stats.files_cached,
                ms: usage.stats.elapsed_ms,
              })
            : t("usage.notScanned")}
        </span>
      </div>

      {loadingFirst && (
        <div className="panel px-4 py-6 text-[12px] text-text-dim" data-usage-loading>
          {t("usage.scanningHint")}
        </div>
      )}

      {!loadingFirst && empty && (
        <Panel>
          <Empty
            title={t("usage.emptyTitle")}
            hint={t("usage.emptyHint")}
            action={
              <Button variant="outline" size="sm" onClick={() => void refreshUsage(range, true)}>
                {t("usage.rescan")}
              </Button>
            }
          />
        </Panel>
      )}

      {!loadingFirst && view && !empty && (
        <>
          <UsageMetrics totals={view.totals} />

          <div className="grid grid-cols-1 gap-3 xl:grid-cols-[minmax(0,1fr)_320px]">
            <div className="flex min-w-0 flex-col gap-3">
              <Panel
                title={t("usage.trend")}
                subtitle={
                  usage?.granularity === "hour"
                    ? t("usage.trendHourly")
                    : t("usage.trendDaily")
                }
              >
                <UsageTrend points={view.series} granularity={usage?.granularity ?? "day"} />
              </Panel>

              <Panel
                title={t("usage.modelDetail")}
                subtitle={t("usage.modelDetailHint")}
                bodyClassName="px-2 py-2"
              >
                <ModelTable
                  rows={sortedModels}
                  providerOf={(group) => modelProvider.get(group.key) ?? "—"}
                  sortKey={sortKey}
                  sortDir={sortDir}
                  onSort={onSort}
                  onPick={(model) => toggleIn("models", model)}
                />
              </Panel>
            </div>

            <aside className="flex min-w-0 flex-col gap-3">
              <Panel
                title={t("usage.byAgent")}
                subtitle={t("usage.byAgentHint")}
                actions={
                  <Button variant="ghost" size="sm" onClick={() => setPricingOpen(true)}>
                    <Tags className="h-3.5 w-3.5" />
                    {t("usage.pricing")}
                  </Button>
                }
              >
                <ProviderBreakdown
                  groups={usage?.by_agent ?? []}
                  total={usage?.totals.total ?? 0}
                  active={filter.agents}
                  onToggle={(key) => toggleIn("agents", key)}
                  emptyHint={t("usage.noAgent")}
                  iconOf={(key) => <AgentIcon id={key} className="h-3.5 w-3.5" />}
                />
              </Panel>

              <Panel title={t("usage.byProvider")} subtitle={t("usage.byProviderHint")}>
                <ProviderBreakdown
                  groups={usage?.by_provider ?? []}
                  total={usage?.totals.total ?? 0}
                  active={filter.providers}
                  onToggle={(key) => toggleIn("providers", key)}
                  emptyHint={t("usage.noProvider")}
                />
              </Panel>

              <Panel title={t("usage.sources")} subtitle={t("usage.sourcesHint")}>
                <SourcePanel sources={usage?.sources ?? []} />
              </Panel>

              <p className="px-1 text-[11px] leading-relaxed text-text-dim">{usage?.note}</p>
            </aside>
          </div>
        </>
      )}

      {pricingOpen && (
        <PricingDialog onClose={() => setPricingOpen(false)} notify={notify} />
      )}
    </div>
  );
}
