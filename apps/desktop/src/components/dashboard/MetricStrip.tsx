import { Activity, Boxes, HardDrive, Link2 } from "lucide-react";
import { clsx } from "clsx";
import type { DashboardMetrics } from "../../lib/dashboard";
import { bytes } from "../../lib/format";
import { useT } from "../../lib/i18n";
import type { MessageKey } from "../../lib/i18n/messages";

/**
 * 总览抬头右侧的四格指标条：细边 + 竖分隔线 + 等宽数字。
 * 数值为 0 时照常显示 0，不用占位符，避免「有数据但看起来像没扫」。
 */
export function MetricStrip({ metrics }: { metrics: DashboardMetrics }) {
  const t = useT();
  const cells: {
    key: string;
    icon: typeof Boxes;
    value: string;
    label: MessageKey;
    accent?: boolean;
  }[] = [
    { key: "records", icon: Boxes, value: String(metrics.records), label: "dash.metric.records" },
    { key: "agents", icon: Activity, value: String(metrics.agents), label: "dash.metric.agents" },
    { key: "groups", icon: Link2, value: String(metrics.groups), label: "dash.metric.groups" },
    {
      key: "wasted",
      icon: HardDrive,
      value: bytes(metrics.wastedBytes),
      label: "dash.metric.wasted",
      accent: true,
    },
  ];

  return (
    <div
      data-metric-strip
      className="flex shrink-0 items-stretch overflow-hidden rounded-lg border border-line bg-ink-900/60"
      aria-label={t("dash.metric.aria")}
    >
      {cells.map(({ key, icon: Icon, value, label, accent }, index) => (
        <div
          key={key}
          className={clsx(
            "flex min-h-[46px] items-center gap-2 px-3",
            index > 0 && "border-l border-line"
          )}
        >
          <Icon className={clsx("h-3.5 w-3.5 shrink-0", accent ? "text-brass" : "text-text-dim")} />
          <strong
            className={clsx(
              "font-mono text-[13px] font-medium tabular-nums",
              accent ? "text-brass" : "text-text-strong"
            )}
          >
            {value}
          </strong>
          <span className="whitespace-nowrap text-[11.5px] text-text-dim">{t(label)}</span>
        </div>
      ))}
    </div>
  );
}
