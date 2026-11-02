import { useEffect, useMemo, useState, type ReactNode } from "react";
import { clsx } from "clsx";
import { RefreshCw } from "lucide-react";
import { AgentRow } from "../components/agents/AgentRow";
import { Badge, Button, Empty } from "../components/ui";
import { useHub } from "../lib/useHub";
import { useContextMenu, useMenuEnv } from "../lib/context-menu";
import { composeMenu } from "../lib/menu-builders/common";
import { agentRowMenu, agentsPageMenu } from "../lib/menu-builders/agents";
import { ConfirmDialog } from "../components/workbench/ConfirmDialog";
import { countStates, fromAgentInstalls, groupCatalog } from "../lib/agent-catalog";
import { useT } from "../lib/i18n";
import type { MessageKey } from "../lib/i18n/messages";
import type { CatalogEntry, EnvState } from "../lib/types";

type Filter = EnvState | "all";

const FILTERS: { key: Filter; label: MessageKey }[] = [
  { key: "all", label: "agents.filter.all" },
  { key: "ready", label: "agents.filter.ready" },
  { key: "config_only", label: "agents.filter.configOnly" },
  { key: "missing", label: "agents.filter.missing" },
];

export function Agents({
  focusKey,
  onInspectAgent,
}: {
  focusKey?: string | null;
  /** shared 条目的「打开 Codex 技能」：复用技能库的按 agent 筛选 */
  onInspectAgent?: (agentId: string) => void;
}) {
  const {
    catalog,
    catalogLoading,
    overview,
    refreshCatalog,
    addManagedAgent,
    removeManagedAgent,
    notify,
  } = useHub();
  const t = useT();
  const menuEnv = useMenuEnv();
  const contextMenu = useContextMenu();
  const [filter, setFilter] = useState<Filter>("all");
  const [expanded, setExpanded] = useState<string | null>(null);
  const [pendingAdd, setPendingAdd] = useState<string | null>(null);
  const [pendingRemove, setPendingRemove] = useState<CatalogEntry | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  // 老版本桌面壳没有 hub_agent_catalog：退回扫描结果里的 5 条内置记录
  const degraded = catalog.length === 0;
  const entries = useMemo<CatalogEntry[]>(
    () => (catalog.length > 0 ? catalog : fromAgentInstalls(overview?.report.agents ?? [])),
    [catalog, overview]
  );
  const counts = useMemo(() => countStates(entries), [entries]);
  const platform = entries.find((entry) => entry.platform)?.platform ?? "";

  useEffect(() => {
    if (!focusKey) return;
    setFilter("all");
    setExpanded(focusKey);
    const node = document.querySelector(`[data-agent-row="${focusKey}"]`);
    node?.scrollIntoView({ block: "center", behavior: "smooth" });
  }, [focusKey]);

  const countFor = (key: Filter): number => {
    if (key === "all") return counts.total;
    if (key === "ready") return counts.ready;
    if (key === "config_only") return counts.configOnly;
    return counts.missing;
  };

  const visible = filter === "all" ? entries : entries.filter((entry) => entry.state === filter);
  const groups = groupCatalog(visible);

  const confirmAdd = (key: string) => {
    setBusy(key);
    void addManagedAgent(key).finally(() => {
      setBusy(null);
      setPendingAdd(null);
    });
  };

  const remove = (key: string) => {
    setBusy(key);
    void removeManagedAgent(key).finally(() => setBusy(null));
  };

  const toggle = (key: string) => {
    setPendingAdd(null);
    setExpanded((prev) => (prev === key ? null : key));
  };

  const openRowMenu = (event: React.MouseEvent, entry: CatalogEntry) => {
    contextMenu.open(
      event,
      composeMenu(
        agentRowMenu({
          entry,
          openSteps: () => toggle(entry.key),
          requestAdd: () => {
            setExpanded(entry.key);
            setPendingAdd(entry.key);
          },
          confirmAdd: () => confirmAdd(entry.key),
          requestRemove: () => setPendingRemove(entry),
          pendingAdd: pendingAdd === entry.key,
          env: menuEnv,
          notify,
        }),
        menuEnv,
        "agents"
      )
    );
  };

  const openPageMenu = (event: React.MouseEvent) => {
    contextMenu.open(
      event,
      composeMenu(
        agentsPageMenu({
          probe: () => void refreshCatalog(true),
          probing: catalogLoading,
        }),
        menuEnv,
        "agents"
      )
    );
  };

  const rowProps = (entry: CatalogEntry) => ({
    entry,
    expanded: expanded === entry.key,
    pendingAdd: pendingAdd === entry.key,
    busy: busy === entry.key,
    onToggle: () => toggle(entry.key),
    onRequestAdd: () => {
      setExpanded(entry.key);
      setPendingAdd(entry.key);
    },
    onCancelAdd: () => setPendingAdd(null),
    onConfirmAdd: () => confirmAdd(entry.key),
    onRemove: () => remove(entry.key),
    onOpenShared: (agentId: string) => onInspectAgent?.(agentId),
    onContextMenu: (event: React.MouseEvent) => openRowMenu(event, entry),
  });

  return (
    <div onContextMenu={openPageMenu}>
      <header className="mb-3 flex flex-wrap items-start justify-between gap-2">
        <div>
          <h1 className="text-[13.5px] font-semibold text-text-strong">{t("agents.title")}</h1>
          <p className="mt-0.5 text-[11.5px] text-text-dim">{t("agents.subtitle")}</p>
        </div>
        <div className="flex items-center gap-2">
          {platform && <Badge tone="dim">{platform}</Badge>}
          <Button
            size="sm"
            variant="outline"
            onClick={() => void refreshCatalog(true)}
            disabled={catalogLoading || degraded}
            title={degraded ? t("agents.probeDegraded") : t("agents.probeTitle")}
          >
            <RefreshCw className={clsx("h-3 w-3", catalogLoading && "animate-spin")} />
            {t("agents.probe")}
          </Button>
        </div>
      </header>

      {degraded && (
        <div className="mb-3 rounded-md border border-state-warn/40 bg-state-warn/10 px-3 py-2 text-[11px] leading-relaxed text-state-warn">
          {t("agents.degraded")}
        </div>
      )}

      <div className="panel mb-3 flex items-stretch overflow-hidden">
        {FILTERS.map((item) => (
          <button
            key={item.key}
            type="button"
            onClick={() => setFilter(item.key)}
            aria-pressed={filter === item.key}
            className={clsx(
              "flex flex-1 cursor-pointer flex-col items-center gap-0.5 border-l border-line px-3 py-2 transition-colors first:border-l-0",
              filter === item.key
                ? "bg-surface-2 text-text-strong"
                : "text-text-muted hover:bg-surface-hover"
            )}
          >
            <span className="mono text-[15px] font-semibold">{countFor(item.key)}</span>
            <span className="text-[11.5px] text-text-dim">{t(item.label)}</span>
          </button>
        ))}
      </div>

      {visible.length === 0 ? (
        <div className="panel">
          <Empty
            title={t("agents.emptyTitle")}
            hint={t("agents.emptyHint")}
            action={
              <Button size="sm" variant="outline" onClick={() => setFilter("all")}>
                {t("agents.viewAll")}
              </Button>
            }
          />
        </div>
      ) : (
        <div className="flex flex-col gap-3">
          {groups.managed.length > 0 && (
            <Group
              title={t("agents.group.managed")}
              hint={t("agents.group.managedHint")}
            >
              {groups.managed.map((entry) => (
                <AgentRow key={entry.key} {...rowProps(entry)} />
              ))}
            </Group>
          )}
          {groups.catalog.length > 0 && (
            <Group
              title={t("agents.group.catalog")}
              hint={t("agents.group.catalogHint")}
            >
              {groups.catalog.map((entry) => (
                <AgentRow key={entry.key} {...rowProps(entry)} />
              ))}
            </Group>
          )}
        </div>
      )}

      <p className="mt-3 text-[11px] text-text-dim">{t("agents.footnote")}</p>

      <ConfirmDialog
        open={pendingRemove !== null}
        title={t("agents.removeConfirm")}
        body={pendingRemove ? t("agents.removeBody", { name: pendingRemove.name }) : ""}
        confirmLabel={t("agents.managed.remove")}
        onCancel={() => setPendingRemove(null)}
        onConfirm={() => {
          const entry = pendingRemove;
          setPendingRemove(null);
          if (entry) remove(entry.key);
        }}
      />
    </div>
  );
}

function Group({
  title,
  hint,
  children,
}: {
  title: string;
  hint: string;
  children: ReactNode;
}) {
  return (
    <section className="panel overflow-hidden">
      <header className="flex items-baseline gap-2 border-b border-line px-3 py-2">
        <h2 className="text-[12px] font-medium text-text-strong">{title}</h2>
        <span className="text-[11.5px] text-text-dim">{hint}</span>
      </header>
      {children}
    </section>
  );
}
