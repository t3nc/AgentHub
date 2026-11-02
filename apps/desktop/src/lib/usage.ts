import type { MessageKey } from "./i18n/messages";
import { tr } from "./i18n/runtime";
import type {
  ModelPrice,
  UsageCell,
  UsageGranularity,
  UsageGroup,
  UsageRange,
  UsageTotals,
} from "./types";

/** 六档时间范围。顺序即分段控件的显示顺序。 */
export const RANGE_OPTIONS: { key: UsageRange; label: MessageKey; hint: MessageKey }[] = [
  { key: "today", label: "usage.range.today", hint: "usage.range.todayHint" },
  { key: "last24h", label: "usage.range.last24h", hint: "usage.range.last24hHint" },
  { key: "days7", label: "usage.range.days7", hint: "usage.range.days7Hint" },
  { key: "days15", label: "usage.range.days15", hint: "usage.range.days15Hint" },
  { key: "days30", label: "usage.range.days30", hint: "usage.range.days30Hint" },
  { key: "all", label: "usage.range.all", hint: "usage.range.allHint" },
];

/**
 * 「没有登记 provider」的展示名。
 *
 * 只是显示值：筛选在客户端算，这个字符串不会被回传给后端当查询条件。
 */
export function unregisteredProvider(): string {
  return tr("usage.unregistered");
}

/**
 * 后端 `api::usage::UNREGISTERED_PROVIDER_ID` 的稳定哨兵，**只用来跟分组键比对**，
 * 不要拿它显示。显示一律走 `unregisteredProvider()`（后端 `by_provider[].label`
 * 已经按语言取好词了）。
 *
 * 后端拿它做 `by_provider` 的 key、后端 `narrow()` 也认它；这个常量必须与后端一致，
 * 否则英文界面下「未登记」那一组会对不上。
 */
export const UNREGISTERED_PROVIDER_ID = "__unregistered__";

export interface UsageFilter {
  agents: string[];
  providers: string[];
  models: string[];
}

export const EMPTY_FILTER: UsageFilter = { agents: [], providers: [], models: [] };

export function emptyTotals(currency = "USD"): UsageTotals {
  return {
    input: 0,
    cache_read: 0,
    cache_write: 0,
    output: 0,
    calls: 0,
    total: 0,
    cost: null,
    unpriced_calls: 0,
    currency,
  };
}

/** 单元格的 provider 展示键：null → 「未登记」。 */
export function providerKey(provider: string | null): string {
  return provider && provider.trim() ? provider : unregisteredProvider();
}

/**
 * 查价格：先按 (model, provider) 精确匹配，再退回「该模型的默认价」。
 * 与 `ext::pricing::lookup` 同一规则，前端筛选后重算才不会与后端给出的数不一致。
 */
export function priceFor(
  prices: ModelPrice[],
  model: string,
  provider: string | null
): ModelPrice | null {
  const want = model.trim().toLowerCase();
  const exact = prices.find(
    (price) =>
      price.model.trim().toLowerCase() === want &&
      (provider
        ? (price.provider ?? "").toLowerCase() === provider.toLowerCase()
        : !price.provider)
  );
  if (exact) return exact;
  return (
    prices.find(
      (price) => price.model.trim().toLowerCase() === want && !price.provider
    ) ?? null
  );
}

const PRICE_UNIT = 1_000_000;

export function costOf(price: ModelPrice, cell: UsageCell): number {
  return (
    (price.input * cell.input +
      price.cache_read * cell.cache_read +
      price.cache_write * cell.cache_write +
      price.output * cell.output) /
    PRICE_UNIT
  );
}

function addCell(totals: UsageTotals, cell: UsageCell, price: ModelPrice | null) {
  totals.input += cell.input;
  totals.cache_read += cell.cache_read;
  totals.cache_write += cell.cache_write;
  totals.output += cell.output;
  totals.calls += cell.calls;
  totals.total = totals.input + totals.cache_read + totals.cache_write + totals.output;
  if (price) {
    totals.cost = (totals.cost ?? 0) + costOf(price, cell);
  } else {
    totals.unpriced_calls += cell.calls;
  }
}

export function filterCells(cells: UsageCell[], filter: UsageFilter): UsageCell[] {
  const agents = new Set(filter.agents);
  const providers = new Set(filter.providers);
  const models = new Set(filter.models);
  return cells.filter(
    (cell) =>
      (agents.size === 0 || agents.has(cell.agent)) &&
      (providers.size === 0 || providers.has(providerKey(cell.provider))) &&
      (models.size === 0 || models.has(cell.model))
  );
}

export function totalsOf(
  cells: UsageCell[],
  prices: ModelPrice[],
  currency = "USD"
): UsageTotals {
  const totals = emptyTotals(currency);
  for (const cell of cells) {
    addCell(totals, cell, priceFor(prices, cell.model, cell.provider));
  }
  return totals;
}

export interface UsageSeriesPoint {
  bucket: number;
  input: number;
  cache_read: number;
  cache_write: number;
  output: number;
  total: number;
}

/** 趋势用：按桶（小时或天）求和，按时间升序。 */
export function seriesOf(cells: UsageCell[]): UsageSeriesPoint[] {
  const map = new Map<number, UsageSeriesPoint>();
  for (const cell of cells) {
    const point =
      map.get(cell.bucket) ??
      {
        bucket: cell.bucket,
        input: 0,
        cache_read: 0,
        cache_write: 0,
        output: 0,
        total: 0,
      };
    point.input += cell.input;
    point.cache_read += cell.cache_read;
    point.cache_write += cell.cache_write;
    point.output += cell.output;
    point.total = point.input + point.cache_read + point.cache_write + point.output;
    map.set(cell.bucket, point);
  }
  return [...map.values()].sort((a, b) => a.bucket - b.bucket);
}

/**
 * 按某个键聚合成表格行。
 *
 * `labelOf` 负责把 key 变成展示名（agent 用来源 label，provider 直接显示 key）。
 */
export function groupCells(
  cells: UsageCell[],
  keyOf: (cell: UsageCell) => string,
  labelOf: (key: string) => string,
  prices: ModelPrice[],
  currency = "USD"
): UsageGroup[] {
  const map = new Map<string, UsageTotals>();
  for (const cell of cells) {
    const key = keyOf(cell);
    const totals = map.get(key) ?? emptyTotals(currency);
    addCell(totals, cell, priceFor(prices, cell.model, cell.provider));
    map.set(key, totals);
  }
  return [...map.entries()]
    .map(([key, totals]) => ({ key, label: labelOf(key), ...totals }))
    .sort((a, b) => b.total - a.total);
}

export type UsageSortKey =
  | "model"
  | "provider"
  | "input"
  | "cache_read"
  | "cache_write"
  | "output"
  | "calls"
  | "cost";

export type SortDirection = "asc" | "desc";

/** 表格排序。数值列用差值，文本列用本地化比较；缺成本排在最后。 */
export function sortGroups(
  groups: UsageGroup[],
  key: UsageSortKey,
  direction: SortDirection,
  providerOf?: (group: UsageGroup) => string
): UsageGroup[] {
  const sign = direction === "asc" ? 1 : -1;
  const rows = [...groups];
  rows.sort((a, b) => {
    if (key === "model") return a.label.localeCompare(b.label, "zh-CN") * sign;
    if (key === "provider") {
      const left = providerOf ? providerOf(a) : a.key;
      const right = providerOf ? providerOf(b) : b.key;
      return left.localeCompare(right, "zh-CN") * sign;
    }
    if (key === "cost") {
      const left = a.cost;
      const right = b.cost;
      if (left === null || left === undefined) return 1;
      if (right === null || right === undefined) return -1;
      return (left - right) * sign;
    }
    return (a[key] - b[key]) * sign;
  });
  return rows;
}

function pad(value: number): string {
  return String(value).padStart(2, "0");
}

/** 桶的时间标签：小时档到点，天档到日。 */
export function bucketLabel(bucket: number, granularity: UsageGranularity): string {
  const date = new Date(bucket * 1000);
  if (granularity === "hour") {
    return `${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:00`;
  }
  return `${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

/**
 * 趋势图 x 轴刻度用标签：比 `bucketLabel` 短一号。
 *
 * 小时档只留 `HH:00`（日期已经在区间说明里写过，刻度上再带日期会互相压字）；
 * 天档保持 `MM-DD`。首尾两个刻度也用它，保证同一行字号/格式一致。
 */
export function tickLabel(bucket: number, granularity: UsageGranularity): string {
  const date = new Date(bucket * 1000);
  if (granularity === "hour") return `${pad(date.getHours())}:00`;
  return `${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

/**
 * 挑出要画刻度的下标：**首尾必含**，中间均匀取，结果去重后升序。
 *
 * 只依赖点数，不看容器宽度——容器宽度要读 `getBoundingClientRect()`，
 * 而窄屏收起中间刻度由 CSS（`hidden md:inline`）负责，这样同一份数据
 * 在演示模式、无头浏览器与真实窗口里算出的刻度完全一致。
 */
export function tickIndexes(count: number, maxTicks: number): number[] {
  if (count <= 0) return [];
  if (maxTicks <= 1 || count === 1) return [0];
  if (count <= maxTicks) return Array.from({ length: count }, (_, index) => index);
  const step = (count - 1) / (maxTicks - 1);
  const picked = new Set<number>();
  for (let i = 0; i < maxTicks; i += 1) {
    picked.add(Math.round(i * step));
  }
  picked.add(0);
  picked.add(count - 1);
  return [...picked].sort((a, b) => a - b);
}

/** 完整日期（用于区间说明）。 */
export function dayLabel(epochSeconds: number | null): string {
  if (!epochSeconds) return "—";
  const date = new Date(epochSeconds * 1000);
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

/** 该范围是否按小时出点（趋势图的 x 轴密度不同）。 */
export function isHourly(granularity: UsageGranularity): boolean {
  return granularity === "hour";
}
