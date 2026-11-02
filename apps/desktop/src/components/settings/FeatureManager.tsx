import { AlertTriangle, Network, Receipt } from "lucide-react";
import { clsx } from "clsx";
import { Badge, Toggle } from "../ui";
import { useHub } from "../../lib/useHub";
import { tr } from "../../lib/i18n/runtime";
import type { FeatureKey, FeatureMeta } from "../../lib/types";

const ICONS: Record<FeatureKey, typeof Network> = {
  provider: Network,
  usage: Receipt,
};

/**
 * 开关文案的兜底副本。
 *
 * 正常路径用后端 `feature_catalog` 下发的名字与说明（CLI / 禁用页共用一份），
 * 这里只在设置视图还没读回来时占位，避免首帧出现空白行。
 */
function fallbackCatalog(): FeatureMeta[] {
  return [
    {
      key: "provider",
      display_name: tr("feat.provider"),
      description: tr("feat.providerDesc"),
      enabled: true,
    },
    {
      key: "usage",
      display_name: tr("feat.usage"),
      description: tr("feat.usageDesc"),
      enabled: true,
    },
  ];
}

/**
 * 功能管理。
 *
 * 开关即时生效（写 config.json），不参与页面底部的「保存并重扫」：
 * 混进全量提交会被 dirty 状态牵着走，而用户期望点一下就从导航消失。
 */
export function FeatureManager() {
  const { settings, features, setFeature } = useHub();
  const rows = settings?.feature_catalog?.length
    ? settings.feature_catalog
    : fallbackCatalog();
  const providerOff = !features.provider;

  return (
    <div className="divide-y divide-line">
      <div className="px-3 py-2.5">
        <p className="text-[11.5px] leading-relaxed text-text-dim">
          {tr("feat.note1")}
          {tr("feat.note2")}
        </p>
      </div>

      {rows.map((row) => {
        const Icon = ICONS[row.key];
        const enabled = features[row.key] ?? row.enabled;
        return (
          <div
            key={row.key}
            className={clsx(
              "flex items-start justify-between gap-4 px-3 py-3 transition-colors",
              enabled ? "" : "bg-surface-2/40",
              "hover:bg-surface-hover/40"
            )}
          >
            <div className="flex min-w-0 items-start gap-2.5">
              <span
                className={clsx(
                  "mt-0.5 flex h-7 w-7 shrink-0 items-center justify-center rounded-lg border",
                  enabled
                    ? "border-brand-500/30 bg-brand-500/10 text-brand-300"
                    : "border-line bg-surface-2 text-text-dim"
                )}
              >
                <Icon className="h-3.5 w-3.5" />
              </span>
              <div className="min-w-0">
                <div className="flex items-center gap-2">
                  <p
                    className={clsx(
                      "text-[12px] font-medium",
                      enabled ? "text-text-strong" : "text-text-muted"
                    )}
                  >
                    {row.display_name}
                  </p>
                  <Badge tone={enabled ? "ok" : "dim"}>
                    {enabled ? tr("feat.enabled") : tr("feat.disabled")}
                  </Badge>
                </div>
                <p className="mt-0.5 text-[11px] leading-relaxed text-text-dim">
                  {row.description}
                </p>
                {row.key === "provider" && providerOff && (
                  <p className="mt-1.5 flex items-start gap-1.5 text-[11px] leading-relaxed text-state-warn">
                    <AlertTriangle className="mt-[1px] h-3 w-3 shrink-0" />
                    <span>{tr("feat.gatewayWarn")}</span>
                  </p>
                )}
                {row.key === "usage" && !enabled && (
                  <p className="mt-1.5 text-[11px] leading-relaxed text-text-dim">
                    {tr("feat.usageWarn")}
                  </p>
                )}
              </div>
            </div>
            <div className="flex shrink-0 items-center pt-1">
              <Toggle checked={enabled} onChange={(next) => void setFeature(row.key, next)} />
            </div>
          </div>
        );
      })}
    </div>
  );
}
