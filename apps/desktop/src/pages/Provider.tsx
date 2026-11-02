import { useEffect, useMemo, useState } from "react";
import { clsx } from "clsx";
import { ChevronDown, Plus } from "lucide-react";
import { Badge, Button, Empty } from "../components/ui";
import { useHub } from "../lib/useHub";
import { AgentRail } from "../components/provider/AgentRail";
import { ProviderPool } from "../components/provider/ProviderPool";
import { ModelSlotDrawer } from "../components/provider/ModelSlotDrawer";
import { ProviderPanel } from "../components/provider/ProviderPanel";
import { RequestStream } from "../components/provider/RequestStream";
import { RouteCard } from "../components/provider/RouteCard";
import { RouteEditor } from "../components/provider/RouteEditor";
import { SupplierWizard } from "../components/provider/SupplierWizard";
import { useT } from "../lib/i18n";
import type { MessageKey } from "../lib/i18n/messages";
import type { GatewayRouteView, ModelSlots, PoolMode, ProviderView } from "../lib/types";

type AdvancedTab = "suppliers" | "routes" | "stream";

const ADVANCED_TABS: { key: AdvancedTab; label: MessageKey }[] = [
  { key: "suppliers", label: "provider.tab.suppliers" },
  { key: "routes", label: "provider.tab.routes" },
  { key: "stream", label: "provider.tab.stream" },
];

/**
 * 供应商工作台。
 *
 * 左栏选 agent，右栏是它自己的供应商池 —— 对齐 CC Switch 的心智模型：
 * 一屏看清「这个 agent 现在用谁」，点一下就换。
 */
export function Provider() {
  const {
    settings,
    refreshSettings,
    gateway,
    gatewayStatus,
    refreshGateway,
    refreshGatewayStatus,
    refreshGatewayLogs,
    applyReports,
    addToPool,
    removeFromPool,
    setDefaultProvider,
    setPoolMode,
    setModelSlots,
    startGateway,
    stopGateway,
  } = useHub();
  const t = useT();

  const [active, setActive] = useState("claude");
  const [busy, setBusy] = useState<string | null>(null);
  const [advanced, setAdvanced] = useState(false);
  const [tab, setTab] = useState<AdvancedTab>("suppliers");
  const [wizard, setWizard] = useState<{ open: boolean; editing: ProviderView | null }>({
    open: false,
    editing: null,
  });
  const [slotsOpen, setSlotsOpen] = useState(false);
  const [editor, setEditor] = useState<{ open: boolean; route: GatewayRouteView | null }>({
    open: false,
    route: null,
  });

  useEffect(() => {
    if (!settings) void refreshSettings();
    void refreshGateway();
    void refreshGatewayStatus();
    void refreshGatewayLogs(200);
  }, [settings, refreshSettings, refreshGateway, refreshGatewayStatus, refreshGatewayLogs]);

  // 运行中才轮询请求流：停着的时候不会有新行
  useEffect(() => {
    if (!gatewayStatus?.running) return;
    const timer = window.setInterval(() => void refreshGatewayLogs(200), 2000);
    return () => window.clearInterval(timer);
  }, [gatewayStatus?.running, refreshGatewayLogs]);

  const providers = settings?.providers.providers ?? [];
  const bindings = settings?.providers.bindings ?? [];
  const current = useMemo(
    () => bindings.find((item) => item.agent === active) ?? bindings[0],
    [bindings, active]
  );
  const currentProvider = providers.find((item) => item.id === current?.provider_id) ?? null;
  /**
   * 当前 agent 的网关路由：池卡片上的 ×权重 / #优先级 从这里取。
   * 直连时 `binding.route_id` 为 null（后端有意清掉），此时 route 也就是 null。
   */
  const poolRoute = useMemo(
    () => gateway?.routes.find((item) => item.id === current?.route_id) ?? null,
    [gateway, current?.route_id]
  );

  const run = async (key: string, action: () => Promise<unknown>) => {
    setBusy(key);
    try {
      await action();
    } finally {
      setBusy(null);
    }
  };

  const modes = bindings.filter((item) => item.provider_id).length;

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3">
      <header className="flex flex-wrap items-center justify-between gap-2">
        <div>
          <h1 className="text-[13.5px] font-semibold text-text-strong">
            {t("provider.title")}
          </h1>
          <p className="mt-0.5 text-[11.5px] text-text-dim">{t("provider.subtitle")}</p>
        </div>
        <div className="flex flex-wrap items-center gap-1.5">
          <Badge tone="brand">{t("provider.count", { count: providers.length })}</Badge>
          <Badge tone={modes > 0 ? "ok" : "dim"}>
            {t("provider.agentsConfigured", { count: modes })}
          </Badge>
          {gatewayStatus?.running && (
            <Badge tone="warn">{t("provider.gatewayPort", { port: gatewayStatus.port })}</Badge>
          )}
        </div>
      </header>

      <div className="panel flex min-h-0 flex-1 overflow-hidden">
        {settings && current ? (
          <>
            <AgentRail
              bindings={bindings}
              active={current.agent}
              onSelect={setActive}
              gatewayRunning={Boolean(gatewayStatus?.running)}
              onStartGateway={() => void startGateway()}
              onStopGateway={() => void stopGateway()}
            />
            <ProviderPool
              binding={current}
              providers={providers}
              route={poolRoute}
              report={applyReports[current.agent]}
              busy={busy}
              gatewayRunning={Boolean(gatewayStatus?.running)}
              onAdd={(id) => void run(`add:${id}`, () => addToPool(current.agent, id))}
              onRemove={(id) => void run(`member:${id}`, () => removeFromPool(current.agent, id))}
              onSetDefault={(id) =>
                void run(`member:${id}`, () => setDefaultProvider(current.agent, id))
              }
              onMode={(mode: PoolMode) =>
                void run("mode", () => setPoolMode(current.agent, mode))
              }
              onOpenSlots={() => setSlotsOpen(true)}
              onAddProvider={() => setWizard({ open: true, editing: null })}
              onEdit={(provider) => setWizard({ open: true, editing: provider })}
              onStartGateway={() => void startGateway()}
            />
          </>
        ) : (
          <Empty
            title={settings ? t("provider.emptyTitle") : t("provider.loadingTitle")}
            hint={settings ? t("provider.noAgentHint") : t("provider.loadingHint")}
          />
        )}
      </div>

      <div className="panel overflow-hidden">
        <button
          type="button"
          onClick={() => setAdvanced(!advanced)}
          className="flex w-full cursor-pointer items-center gap-2 px-3 py-2 text-left transition-colors hover:bg-surface-hover/40"
        >
          <ChevronDown
            className={clsx(
              "h-3.5 w-3.5 text-text-dim transition-transform duration-120",
              advanced && "rotate-180"
            )}
          />
          <span className="text-[12px] font-medium text-text-strong">
            {t("provider.advanced")}
          </span>
          <span className="text-[11px] text-text-dim">{t("provider.advancedHint")}</span>
          <span className="ml-auto text-[11px] text-text-dim">
            {t("provider.routesCount", { count: (gateway?.routes ?? []).length })}
          </span>
        </button>

        {advanced && (
          <div className="border-t border-line">
            <nav
              aria-label={t("provider.advancedAria")}
              className="flex flex-wrap items-center gap-1 border-b border-line bg-surface-2/40 px-2 py-1.5"
            >
              {ADVANCED_TABS.map((item) => (
                <button
                  key={item.key}
                  type="button"
                  onClick={() => setTab(item.key)}
                  className={clsx(
                    "cursor-pointer rounded-md px-2.5 py-1 text-[11.5px] transition-colors duration-120",
                    tab === item.key
                      ? "bg-ink-950 text-text-strong shadow-[inset_0_0_0_1px_rgb(var(--line))]"
                      : "text-text-muted hover:bg-surface-hover hover:text-text-strong"
                  )}
                >
                  {t(item.label)}
                </button>
              ))}
              {tab === "routes" && (
                <div className="ml-auto pr-0.5">
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={() => setEditor({ open: true, route: null })}
                    disabled={providers.length === 0}
                    title={
                      providers.length === 0
                        ? t("provider.newRouteHintEmpty")
                        : t("provider.newRouteHint")
                    }
                  >
                    <Plus className="h-3 w-3" />
                    {t("provider.newRoute")}
                  </Button>
                </div>
              )}
            </nav>

            <div className="p-2.5">
              {tab === "suppliers" && settings && (
                <ProviderPanel view={settings.providers} showBindings={false} />
              )}
              {tab === "routes" &&
                ((gateway?.routes ?? []).length === 0 ? (
                  <Empty title={t("provider.noRoutes")} hint={t("provider.noRoutesHint")} />
                ) : (
                  <div className="grid grid-cols-1 gap-2.5 xl:grid-cols-2">
                    {(gateway?.routes ?? []).map((route) => (
                      <RouteCard
                        key={route.id}
                        route={route}
                        providers={providers}
                        onEdit={(next) => setEditor({ open: true, route: next })}
                      />
                    ))}
                  </div>
                ))}
              {tab === "stream" && <RequestStream />}
            </div>
          </div>
        )}
      </div>

      {wizard.open && (
        <SupplierWizard
          editing={wizard.editing}
          providers={providers}
          onClose={() => setWizard({ open: false, editing: null })}
        />
      )}

      {slotsOpen && current && (
        <ModelSlotDrawer
          provider={currentProvider}
          slots={current.model_slots ?? {}}
          mode={current.mode}
          defaultModel={current.model}
          saving={busy === "slots"}
          onClose={() => setSlotsOpen(false)}
          onSave={(slots: ModelSlots) =>
            void run("slots", async () => {
              const report = await setModelSlots(current.agent, slots);
              if (report) setSlotsOpen(false);
            })
          }
        />
      )}

      {editor.open && (
        <RouteEditor
          route={editor.route}
          providers={providers}
          onClose={() => setEditor({ open: false, route: null })}
        />
      )}
    </div>
  );
}
