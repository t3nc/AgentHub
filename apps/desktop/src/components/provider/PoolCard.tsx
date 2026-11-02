import { clsx } from "clsx";
import { Check, Loader2, Minus, MoreHorizontal, Pencil, Plus } from "lucide-react";
import { Badge } from "../ui";
import { useT } from "../../lib/i18n";
import type { ProviderView, RouteMemberView } from "../../lib/types";

function initial(name: string): string {
  const trimmed = name.trim();
  return trimmed ? trimmed.slice(0, 1).toUpperCase() : "?";
}

function hostOf(baseUrl: string): string {
  const rest = baseUrl.trim().replace(/^https?:\/\//, "");
  return rest.split("/")[0] ?? rest;
}

/**
 * 供应商卡片。
 *
 * 三种形态共用一套骨架：`default`（当前生效，带品牌描边）、`member`（已订阅）、
 * `available`（可添加，右侧是「+」）。这样在池里移动时视觉不跳。
 */
export function PoolCard({
  provider,
  state,
  busy = false,
  model,
  member,
  order,
  onAdd,
  onRemove,
  onSetDefault,
  onEdit,
}: {
  provider: ProviderView;
  state: "default" | "member" | "available";
  busy?: boolean;
  model?: string | null;
  /** 网关路由里的同名成员：有它才显示 ×权重 / #优先级 */
  member?: RouteMemberView | null;
  /** 池内序号（从 0 起），没有成员信息时兜底显示顺序 */
  order?: number;
  onAdd?: () => void;
  onRemove?: () => void;
  onSetDefault?: () => void;
  onEdit?: () => void;
}) {
  const t = useT();
  const isDefault = state === "default";
  const weight = member ? Math.max(member.weight, 1) : null;
  const disabled = member ? !member.enabled : false;

  return (
    <div
      className={clsx(
        "group flex items-center gap-2.5 rounded-lg border px-2.5 py-2 transition-colors duration-120",
        isDefault
          ? "border-brand-500/50 bg-brand-500/[0.07]"
          : "border-line bg-surface-2/40 hover:border-brand-500/30 hover:bg-surface-hover/50"
      )}
    >
      <span
        className={clsx(
          "flex h-8 w-8 shrink-0 items-center justify-center rounded-md text-[13px] font-semibold",
          isDefault
            ? "bg-brand-500/20 text-brand-200"
            : "bg-ink-950 text-text-muted"
        )}
      >
        {initial(provider.name)}
      </span>

      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-1.5">
          <span className="truncate text-[12px] font-medium text-text-strong">
            {provider.name}
          </span>
          {isDefault && <Badge tone="brand">{t("pc.default")}</Badge>}
          {disabled && <Badge tone="warn">{t("pc.disabled")}</Badge>}
          {provider.models.length > 0 && (
            <span className="shrink-0 text-[10.5px] text-text-dim">
              {t("panel.models", { count: provider.models.length })}
            </span>
          )}
          {/*
            权重 / 优先级只在池成员上显示（`available` 还没进路由，没有这两个值）。
            口径与 RouteCard / RouteEditor 一致：聚合按权重轮询，路由按优先级升序故障转移。
          */}
          {weight !== null && (
            <span
              data-pool-weight={provider.id}
              title={t("pc.weightHint", { count: weight })}
              className="shrink-0 rounded border border-line px-1 text-[10px] tabular-nums text-text-muted"
            >
              ×{weight}
            </span>
          )}
          {member && (
            <span
              data-pool-priority={provider.id}
              title={t("pc.priorityHint")}
              className="shrink-0 rounded border border-line px-1 text-[10px] tabular-nums text-text-dim"
            >
              #{member.priority}
            </span>
          )}
          {member === null && order !== undefined && (
            <span className="shrink-0 text-[10px] tabular-nums text-text-dim">{order + 1}</span>
          )}
        </div>
        <div className="mt-0.5 flex items-center gap-1.5">
          <a
            href={provider.base_url}
            target="_blank"
            rel="noreferrer"
            onClick={(event) => event.stopPropagation()}
            className="mono truncate text-[10.5px] text-text-dim hover:text-brand-300 hover:underline"
            title={provider.base_url}
          >
            {hostOf(provider.base_url)}
          </a>
          {isDefault && model && (
            <span className="mono shrink-0 truncate text-[10.5px] text-brand-300/80" title={model}>
              · {model}
            </span>
          )}
        </div>
      </div>

      <div className="flex shrink-0 items-center gap-0.5">
        {busy ? (
          <Loader2 className="h-3.5 w-3.5 animate-spin text-text-dim" />
        ) : (
          <>
            {state === "available" && (
              <button
                type="button"
                onClick={onAdd}
                title={t("pc.addToPool")}
                className="flex h-6 w-6 cursor-pointer items-center justify-center rounded text-text-muted transition-colors hover:bg-brand-500/15 hover:text-brand-300"
              >
                <Plus className="h-3.5 w-3.5" />
              </button>
            )}
            {state !== "available" && !isDefault && (
              <button
                type="button"
                onClick={onSetDefault}
                title={t("pc.setDefault")}
                className="flex h-6 w-6 cursor-pointer items-center justify-center rounded text-text-muted transition-colors hover:bg-brand-500/15 hover:text-brand-300"
              >
                <Check className="h-3.5 w-3.5" />
              </button>
            )}
            {state !== "available" && (
              <button
                type="button"
                onClick={onRemove}
                title={t("pc.removeFromPool")}
                className="flex h-6 w-6 cursor-pointer items-center justify-center rounded text-text-muted transition-colors hover:bg-state-danger/15 hover:text-state-danger"
              >
                <Minus className="h-3.5 w-3.5" />
              </button>
            )}
            <button
              type="button"
              onClick={onEdit}
              title={t("pc.editProvider")}
              className="flex h-6 w-6 cursor-pointer items-center justify-center rounded text-text-muted transition-colors hover:bg-surface-hover hover:text-text-strong"
            >
              <Pencil className="h-3.5 w-3.5" />
            </button>
          </>
        )}
      </div>

      {isDefault && (
        <MoreHorizontal className="h-3.5 w-3.5 shrink-0 text-brand-400/60" aria-hidden />
      )}
    </div>
  );
}
