import { useMemo, useState } from "react";
import { clsx } from "clsx";
import { compactTokens, percent } from "../../lib/format";
import { bucketLabel, tickIndexes, tickLabel, type UsageSeriesPoint } from "../../lib/usage";
import { Empty } from "../ui";
import { useT } from "../../lib/i18n";
import type { MessageKey } from "../../lib/i18n/messages";
import type { UsageGranularity } from "../../lib/types";

interface Layer {
  key: "cache_read" | "input" | "output";
  label: MessageKey;
  tone: string;
}

/**
 * 底 → 顶：缓存命中通常是最大一段，放底部更稳。
 *
 * 「新建缓存」本机实测恒为 0，界面不再单列；仍并入输出层计算，保证柱高与合计一致。
 */
const LAYERS: Layer[] = [
  { key: "cache_read", label: "umodel.col.cacheRead", tone: "--ok" },
  { key: "input", label: "umodel.col.input", tone: "--info" },
  { key: "output", label: "umodel.col.output", tone: "--warn" },
];

/** 非零但占比极小的分层也至少画这么高（百分比），否则「有数据却看不见」。 */
const MIN_SEGMENT_PCT = 0.6;

/** 超过这个点数后柱间不再留缝，避免密集数据被缝隙吃掉。 */
const DENSE_THRESHOLD = 45;

function valueOf(point: UsageSeriesPoint, key: Layer["key"]): number {
  if (key === "output") return point.output + point.cache_write;
  return point[key];
}

interface Segment {
  key: string;
  tone: string;
  /** 距柱底的偏移（占图表高度的百分比） */
  bottom: number;
  height: number;
  /** 该点最顶层的非零分段，只有它需要圆角 */
  top: boolean;
}

/** 把一根柱子拆成自下而上的分段；`bottom` 用真实占比累加，避免下限抬高导致整体长高。 */
function segmentsOf(point: UsageSeriesPoint, max: number): Segment[] {
  let topKey: Layer["key"] | null = null;
  for (const layer of LAYERS) {
    if (valueOf(point, layer.key) > 0) topKey = layer.key;
  }

  const out: Segment[] = [];
  let cursor = 0;
  for (const layer of LAYERS) {
    const value = valueOf(point, layer.key);
    if (value <= 0) continue;
    const height = (value / max) * 100;
    out.push({
      key: layer.key,
      tone: layer.tone,
      bottom: cursor,
      height: Math.max(height, MIN_SEGMENT_PCT),
      top: layer.key === topKey,
    });
    cursor += height;
  }
  return out;
}

/**
 * 趋势图：纯 CSS 堆叠柱状图，不引图表库。
 *
 * 每根柱是等宽的一列，分段用绝对定位按百分比高度自底向上堆叠；
 * 悬停按鼠标在容器里的横向比例定位到柱子（密度再高也能命中），浮层用 HTML 叠加。
 */
export function UsageTrend({
  points,
  granularity,
}: {
  points: UsageSeriesPoint[];
  granularity: UsageGranularity;
}) {
  const t = useT();
  const [hover, setHover] = useState<number | null>(null);

  const max = useMemo(() => {
    const peak = points.reduce((acc, point) => Math.max(acc, point.total), 0);
    return Math.max(peak, 1);
  }, [points]);

  const columns = useMemo(() => points.map((point) => segmentsOf(point, max)), [points, max]);
  /** 刻度下标：纯计算，只跟点数走；窄屏收起中间刻度交给 CSS */
  const ticks = useMemo(() => tickIndexes(points.length, 6), [points.length]);

  if (points.length === 0) {
    return (
      <Empty title={t("trend.emptyTitle")} hint={t("usage.emptyHint")} />
    );
  }

  // 切换范围后 points 可能变短，越界的 hover 直接当成没有悬停
  const hovered = hover !== null ? (points[hover] ?? null) : null;
  const hoverPct = hover !== null ? ((hover + 0.5) / points.length) * 100 : 0;
  const dense = points.length > DENSE_THRESHOLD;
  /**
   * 柱顶很高时浮层改挂到底部：它固定挂在图表上方区域，柱子越高越容易被它盖住
   * 最关键的柱顶峰值。阈值取 70%，低于这个高度时浮层仍挂在顶部（不挡柱身）。
   */
  const tooltipAtBottom = hovered !== null && hovered.total >= max * 0.7;

  const handleMove = (event: React.MouseEvent<HTMLDivElement>) => {
    const rect = event.currentTarget.getBoundingClientRect();
    if (rect.width <= 0) return;
    const ratio = Math.min(1, Math.max(0, (event.clientX - rect.left) / rect.width));
    // 每列宽度就是 1/n，floor 直接命中鼠标所在的那根柱子（比按端点插值更准）
    const next = Math.min(points.length - 1, Math.floor(ratio * points.length));
    // 同一列内移动不触发重渲染：密集档（>45 柱）下每次 mousemove 都 setState 会整树重渲染，
    // 表现为高亮/浮层抖动。只有列真的换了才更新。
    setHover((prev) => (prev === next ? prev : next));
  };

  return (
    <div className="relative">
      <div className="mb-2 flex items-center gap-3 text-[11px] text-text-dim">
        {LAYERS.map((layer) => (
          <span key={layer.key} className="inline-flex items-center gap-1.5">
            <span
              className="inline-block h-2 w-2 rounded-[2px]"
              style={{ backgroundColor: `rgb(var(${layer.tone}) / 0.75)` }}
            />
            {t(layer.label)}
          </span>
        ))}
        <span className="ml-auto tabular-nums">
          {t("trend.peak", { value: compactTokens(max) })}
        </span>
      </div>

      <div
        className="relative h-[200px] w-full"
        onMouseMove={handleMove}
        onMouseLeave={() => setHover(null)}
        role="img"
        aria-label={t("trend.aria")}
        data-usage-trend
      >
        {/* 网格：只做参照，不抢鼠标 */}
        <div className="pointer-events-none absolute inset-0">
          {[0.25, 0.5, 0.75].map((ratio) => (
            <div
              key={ratio}
              className="absolute inset-x-0 border-t border-dashed border-line"
              style={{ top: `${ratio * 100}%` }}
            />
          ))}
        </div>

        <div className={clsx("relative flex h-full w-full items-stretch", dense ? "gap-0" : "gap-[2px]")}>
          {points.map((point, index) => {
            const active = hover === index;
            return (
              <div
                key={point.bucket}
                className={clsx(
                  "relative flex h-full min-w-0 flex-1 justify-center transition-opacity",
                  hover !== null && !active && "opacity-65"
                )}
                data-usage-bar={point.bucket}
              >
                <div
                  className={clsx(
                    "relative h-full w-full max-w-[56px] rounded-[3px] transition-colors",
                    active && "bg-surface-hover/50"
                  )}
                >
                  {columns[index].map((segment) => (
                    <div
                      key={segment.key}
                      className={clsx("absolute inset-x-0", segment.top && "rounded-t-[2px]")}
                      style={{
                        bottom: `${segment.bottom}%`,
                        height: `${segment.height}%`,
                        backgroundColor: `rgb(var(${segment.tone}) / 0.72)`,
                      }}
                    />
                  ))}
                </div>
              </div>
            );
          })}
        </div>

        {/* 基线压在最上层，柱子贴底时也有清晰落点 */}
        <div className="pointer-events-none absolute inset-x-0 bottom-0 border-t border-line-strong" />
      </div>

      {/* 浮层跟随柱心；clamp 保证贴边时不出界（左右各留半个宽度）。柱很高时挂到底部，别盖柱顶 */}
      {hovered && (
        <div
          className={clsx(
            "pointer-events-none absolute z-10 w-[190px] -translate-x-1/2 rounded-md border border-line bg-ink-900/95 px-2.5 py-2 text-[11.5px] shadow-panel",
            tooltipAtBottom ? "bottom-8" : "top-7"
          )}
          style={{ left: `clamp(95px, ${hoverPct}%, calc(100% - 95px))` }}
          data-usage-tooltip
          data-usage-tooltip-at={tooltipAtBottom ? "bottom" : "top"}
        >
          <div className="mb-1 text-text-dim">{bucketLabel(hovered.bucket, granularity)}</div>
          <Row
            label={t("umodel.col.input")}
            value={hovered.input}
            color="--info"
            total={hovered.total}
          />
          <Row
            label={t("umodel.col.cacheRead")}
            value={hovered.cache_read}
            color="--ok"
            total={hovered.total}
          />
          <Row
            label={t("umodel.col.output")}
            value={hovered.output}
            color="--warn"
            total={hovered.total}
          />
          <div className="mt-1 flex justify-between border-t border-line pt-1 text-text-strong">
            <span>{t("trend.total")}</span>
            <span className="font-mono tabular-nums">{compactTokens(hovered.total)}</span>
          </div>
        </div>
      )}

      {/*
        x 轴刻度：首尾贴边、中间居中；导航栏窄于一档时用 `hidden md:inline` 收起中间刻度，
        避免在窄容器里互相压字（不读 getBoundingClientRect，保证无头浏览器也能断言）。
      */}
      <div className="relative mt-1 h-4 text-[11px] text-text-dim" data-usage-axis>
        {ticks.map((index) => {
          const point = points[index];
          const first = index === 0;
          const last = index === points.length - 1;
          const edge = first || last;
          return (
            <span
              key={point.bucket}
              data-usage-tick={point.bucket}
              data-usage-tick-edge={edge ? (first ? "first" : "last") : undefined}
              className={clsx(
                "absolute top-0 whitespace-nowrap tabular-nums",
                first && "left-0",
                last && "right-0",
                !edge && "hidden -translate-x-1/2 md:inline"
              )}
              style={edge ? undefined : { left: `${((index + 0.5) / points.length) * 100}%` }}
            >
              {tickLabel(point.bucket, granularity)}
            </span>
          );
        })}
      </div>
    </div>
  );
}

function Row({
  label,
  value,
  color,
  total,
}: {
  label: string;
  value: number;
  color: string;
  total: number;
}) {
  return (
    <div className="flex items-center justify-between gap-3">
      <span className="flex items-center gap-1.5 whitespace-nowrap text-text-dim">
        <span
          className="inline-block h-1.5 w-1.5 rounded-full"
          style={{ backgroundColor: `rgb(var(${color}))` }}
        />
        {label}
      </span>
      <span className="whitespace-nowrap font-mono text-text-strong tabular-nums">
        {compactTokens(value)}
        <span className="ml-1 text-[11px] text-text-dim">{percent(value, total)}</span>
      </span>
    </div>
  );
}
