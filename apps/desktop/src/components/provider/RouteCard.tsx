import { useMemo, useState } from "react";
import { clsx } from "clsx";
import { Pencil, Trash2 } from "lucide-react";
import { Badge, Button, Dot } from "../ui";
import { useHub } from "../../lib/useHub";
import { useT } from "../../lib/i18n";
import type { MessageKey } from "../../lib/i18n/messages";
import type { GatewayRouteView, ProviderView } from "../../lib/types";

const MODE_LABEL: Record<GatewayRouteView["mode"], MessageKey> = {
  single: "route.mode.single",
  route: "route.mode.route",
  aggregate: "route.mode.aggregate",
};

const MODE_TONE: Record<GatewayRouteView["mode"], "dim" | "brand" | "warn"> = {
  single: "dim",
  route: "brand",
  aggregate: "warn",
};

const PROTOCOL_LABEL: Record<GatewayRouteView["protocol"], string> = {
  anthropic: "Anthropic",
  chat_completions: "Chat",
  responses: "Responses",
};

export function RouteCard({
  route,
  providers,
  onEdit,
}: {
  route: GatewayRouteView;
  providers: ProviderView[];
  onEdit: (route: GatewayRouteView) => void;
}) {
  const { saveGatewayRoute, deleteGatewayRoute, notify } = useHub();
  const t = useT();
  const [pendingDelete, setPendingDelete] = useState(false);

  const memberNames = useMemo(
    () =>
      route.members.map((member) => ({
        id: member.provider_id,
        name: providers.find((item) => item.id === member.provider_id)?.name ?? member.provider_id,
        weight: member.weight,
        priority: member.priority,
        enabled: member.enabled,
      })),
    [route.members, providers]
  );

  const totalWeight = memberNames.reduce((sum, member) => sum + Math.max(member.weight, 1), 0);
  const modelCount = useMemo(() => {
    const models = new Set<string>();
    for (const member of route.members) {
      for (const model of member.models) models.add(model);
      for (const key of Object.keys(member.model_map)) models.add(key);
    }
    return models.size;
  }, [route.members]);

  const toggle = () => {
    void saveGatewayRoute({ ...route, enabled: !route.enabled });
  };

  return (
    <article
      className={clsx(
        "panel flex flex-col gap-2 px-3 py-2.5 transition-colors duration-120",
        route.enabled ? "hover:border-brand-500/40" : "opacity-60"
      )}
    >
      <header className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-1.5">
            <Badge tone={MODE_TONE[route.mode]}>{t(MODE_LABEL[route.mode])}</Badge>
            <span className="truncate text-[12.5px] font-medium text-text-strong">{route.name}</span>
          </div>
          <p className="mono mt-1 truncate text-[11px] text-text-dim">
            /gw/{route.id} · {PROTOCOL_LABEL[route.protocol]}
          </p>
        </div>
        <div className="flex shrink-0 items-center gap-0.5">
          <Button
            size="sm"
            variant="ghost"
            onClick={toggle}
            title={route.enabled ? t("card.disable") : t("card.enable")}
          >
            <Dot tone={route.enabled ? "ok" : "dim"} />
          </Button>
          <Button size="sm" variant="ghost" onClick={() => onEdit(route)} title={t("card.edit")}>
            <Pencil className="h-3 w-3" />
          </Button>
          <Button
            size="sm"
            variant="ghost"
            onClick={() => setPendingDelete(true)}
            title={t("card.delete")}
          >
            <Trash2 className="h-3 w-3" />
          </Button>
        </div>
      </header>

      <div className="flex flex-wrap items-center gap-1.5">
        {memberNames.map((member) => (
          <span
            key={member.id}
            title={`${t("card.priorityTitle", { priority: member.priority })}${
              member.enabled ? "" : t("card.disabledSuffix")
            }`}
            className={clsx(
              "flex items-center gap-1.5 rounded-md border px-1.5 py-[2px] text-[11px]",
              member.enabled
                ? "border-line bg-surface-2 text-text-muted"
                : "border-line/60 bg-surface-2/40 text-text-dim line-through"
            )}
          >
            <span className="h-1.5 w-1.5 rounded-full bg-brand-400/80" />
            {member.name}
            {route.mode === "aggregate" && (
              <span className="mono text-text-dim">×{Math.max(member.weight, 1)}</span>
            )}
          </span>
        ))}
      </div>

      {route.mode === "aggregate" && totalWeight > 0 && (
        <div
          className="flex h-1 overflow-hidden rounded-full bg-surface-active"
          title={t("card.weightTitle")}
        >
          {memberNames.map((member) => (
            <span
              key={member.id}
              style={{ width: `${(Math.max(member.weight, 1) / totalWeight) * 100}%` }}
              className="h-full bg-brand-500/70"
            />
          ))}
        </div>
      )}

      <footer className="flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px] text-text-dim">
        <span>{t("card.models", { count: modelCount })}</span>
        <span>{t("card.rules", { count: route.rules.length })}</span>
        <span>{t("card.retry", { count: route.retry.max_attempts })}</span>
        {pendingDelete && (
          <span className="ml-auto flex items-center gap-1.5">
            <Button
              size="sm"
              variant="danger"
              onClick={() =>
                void deleteGatewayRoute(route.id).then((ok) => {
                  setPendingDelete(false);
                  if (!ok) notify(t("card.deleteFailed"));
                })
              }
            >
              {t("card.confirmDelete")}
            </Button>
            <Button size="sm" variant="ghost" onClick={() => setPendingDelete(false)}>
              {t("common.cancel")}
            </Button>
          </span>
        )}
      </footer>
    </article>
  );
}
