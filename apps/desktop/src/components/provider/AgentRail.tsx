import { clsx } from "clsx";
import { Power, RotateCw } from "lucide-react";
import { Badge } from "../ui";
import { useT } from "../../lib/i18n";
import type { MessageKey } from "../../lib/i18n/messages";
import type { PoolMode, ProviderBindingView } from "../../lib/types";

const MODE_LABEL: Record<PoolMode, MessageKey> = {
  direct: "pool.mode.direct",
  route: "pool.mode.route",
  aggregate: "pool.mode.aggregate",
};

const MODE_TONE: Record<PoolMode, "dim" | "brand" | "warn"> = {
  direct: "dim",
  route: "brand",
  aggregate: "warn",
};

function initial(name: string): string {
  const trimmed = name.trim();
  return trimmed ? trimmed.slice(0, 1).toUpperCase() : "?";
}

/**
 * 左栏：agent 列表。
 *
 * 每项显示「这个 agent 现在是什么状态」——模式徽标比供应商名更重要，
 * 因为用户最常问的就是「我到底走没走网关」。
 */
export function AgentRail({
  bindings,
  active,
  onSelect,
  gatewayRunning,
  onStartGateway,
  onStopGateway,
}: {
  bindings: ProviderBindingView[];
  active: string;
  onSelect: (agent: string) => void;
  gatewayRunning: boolean;
  onStartGateway: () => void;
  onStopGateway: () => void;
}) {
  const t = useT();
  return (
    <aside className="flex w-[178px] shrink-0 flex-col border-r border-line">
      <div className="px-2.5 pb-1.5 pt-2 text-[10px] font-semibold uppercase tracking-wider text-text-dim">
        Agent
      </div>

      <nav aria-label={t("rail.agentList")} className="min-h-0 flex-1 overflow-y-auto px-1.5">
        {bindings.map((binding) => {
          const selected = binding.agent === active;
          const mode = binding.mode;
          return (
            <button
              key={binding.agent}
              type="button"
              data-agent-rail={binding.agent}
              onClick={() => onSelect(binding.agent)}
              className={clsx(
                "relative mb-0.5 flex w-full cursor-pointer items-center gap-2 rounded-md px-2 py-[7px] text-left transition-colors duration-120",
                selected
                  ? "bg-brand-500/10 text-text-strong"
                  : "text-text-muted hover:bg-surface-hover/60 hover:text-text-strong",
                !binding.supported && "opacity-50"
              )}
            >
              {selected && (
                <span className="absolute left-0 top-1/2 h-4 w-[2px] -translate-y-1/2 rounded-full bg-brand-400" />
              )}
              <span
                className={clsx(
                  "flex h-5 w-5 shrink-0 items-center justify-center rounded text-[10px] font-semibold",
                  selected ? "bg-brand-500/20 text-brand-200" : "bg-ink-950 text-text-dim"
                )}
              >
                {initial(binding.display_name)}
              </span>
              <span className="min-w-0 flex-1 truncate text-[12px]">
                {binding.display_name}
              </span>
              {binding.supported ? (
                <Badge tone={binding.provider_id ? MODE_TONE[mode] : "dim"}>
                  {binding.provider_id ? t(MODE_LABEL[mode]) : t("rail.unconfigured")}
                </Badge>
              ) : (
                <Badge tone="dim">{t("rail.unsupported")}</Badge>
              )}
            </button>
          );
        })}
      </nav>

      <div className="border-t border-line px-2 py-1.5">
        <div className="mb-1 flex items-center gap-1.5">
          <span
            className={clsx(
              "h-1.5 w-1.5 rounded-full",
              gatewayRunning ? "bg-state-ok" : "bg-text-dim/50"
            )}
          />
          <span className="text-[10.5px] text-text-dim">
            {gatewayRunning ? t("rail.gatewayRunning") : t("rail.gatewayStopped")}
          </span>
        </div>
        <button
          type="button"
          onClick={gatewayRunning ? onStopGateway : onStartGateway}
          className="flex w-full cursor-pointer items-center justify-center gap-1 rounded border border-line py-1 text-[11px] text-text-muted transition-colors hover:border-brand-500/40 hover:text-text-strong"
        >
          {gatewayRunning ? (
            <>
              <Power className="h-3 w-3" />
              {t("rail.stop")}
            </>
          ) : (
            <>
              <RotateCw className="h-3 w-3" />
              {t("rail.start")}
            </>
          )}
        </button>
      </div>
    </aside>
  );
}
