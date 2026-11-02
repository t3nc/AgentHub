import { useMemo, useState } from "react";
import { Wrench } from "lucide-react";
import { clsx } from "clsx";
import { ResourceGraph } from "../components/graph/ResourceGraph";
import { MetricStrip } from "../components/dashboard/MetricStrip";
import { DuplicateTable } from "../components/dashboard/DuplicateTable";
import { OverviewRail } from "../components/dashboard/OverviewRail";
import { PendingIssuesList } from "../components/workbench/PendingIssues";
import { Button } from "../components/ui";
import { useHub } from "../lib/useHub";
import { useT } from "../lib/i18n";
import { useSelection } from "../lib/selection";
import { useContextMenu, useMenuEnv } from "../lib/context-menu";
import { composeMenu } from "../lib/menu-builders/common";
import { agentNodeMenu, dashboardPageMenu, dupRowMenu } from "../lib/menu-builders/dashboard";
import { buildGraph, savingsOf } from "../lib/graph";
import {
  filterGroups,
  metricsOf,
  representativeOf,
  tabCounts,
  type DuplicateTab,
} from "../lib/dashboard";
import { timeAgo } from "../lib/format";
import type { DuplicateGroup } from "../lib/types";

/**
 * 总览页：抬头 + 指标条 → 固定高图谱面板 → 重复技能表 → 右栏详情。
 *
 * 与旧版的差别是「整页可滚动」而不是「全屏画布 + 底部折叠面板」：
 * 窗口高度不再决定图谱大小，表格也不用挤在一个抽屉里。
 */
export function Dashboard({
  onOpenSkills,
  onOpenChanges,
  onOpenSettings,
}: {
  /** 跳到技能库；带 agentId 时按来源筛选 */
  onOpenSkills: (agentId?: string | null) => void;
  onOpenChanges: (batch?: string | null) => void;
  /** 体检项「去设置」的落点（中央仓库路径等装配期问题） */
  onOpenSettings: () => void;
}) {
  const { overview, planDedupe, planRepair, notify } = useHub();
  const t = useT();
  const { select } = useSelection();
  const menuEnv = useMenuEnv();
  const contextMenu = useContextMenu();
  const [tab, setTab] = useState<DuplicateTab>("optimize");
  const [focus, setFocus] = useState<"all" | "issues">("all");
  const [picked, setPicked] = useState<string[]>([]);

  const model = useMemo(() => (overview ? buildGraph(overview) : null), [overview]);
  if (!overview || !model) return null;

  const groups = overview.report.duplicates;
  const counts = tabCounts(groups);
  const visible = filterGroups(groups, tab);
  const metrics = metricsOf(overview);
  const totalWasted = savingsOf(groups);
  // 「异常」只看真正断掉的连接：副本多但链接正常属于可优化，不算异常
  const brokenAgents = model.agents.filter((agent) => agent.health === "danger").length;
  const allPicked = groups.length > 0 && picked.length === groups.length;

  const toggle = (name: string) =>
    setPicked((prev) => (prev.includes(name) ? prev.filter((n) => n !== name) : [...prev, name]));

  const toggleAll = () => setPicked(allPicked ? [] : groups.map((group) => group.name));

  const onSelectGroup = (group: DuplicateGroup) => {
    const record = representativeOf(group);
    if (record) select({ kind: "skill", id: record.path });
  };

  const onPlan = () => {
    // 选了就只处理选中的；没选就是整批（后端会跳过不可处理的组）
    void planDedupe(picked.length > 0 ? picked : undefined);
  };

  const openRowMenu = (event: React.MouseEvent, group: DuplicateGroup) => {
    contextMenu.open(
      event,
      composeMenu(
        dupRowMenu({
          group,
          selected: picked.includes(group.name),
          select: () => onSelectGroup(group),
          toggle: () => toggle(group.name),
          planOne: () => void planDedupe([group.name]),
          env: menuEnv,
          notify,
        }),
        menuEnv,
        "dashboard"
      )
    );
  };

  const pageEntries = () =>
    dashboardPageMenu({
      pickedCount: picked.length,
      planPicked: onPlan,
      repair: () => void planRepair(),
      openSkills: (agentId) => onOpenSkills(agentId),
    });

  const openPageMenu = (event: React.MouseEvent) => {
    contextMenu.open(event, composeMenu(pageEntries(), menuEnv, "dashboard"));
  };

  const openNodeMenu = (event: React.MouseEvent, nodeId: string) => {
    // 中央仓库节点用页面级菜单，agent 节点给它自己的两项
    const entries = nodeId === "store" ? pageEntries() : agentNodeMenu(nodeId, menuEnv);
    contextMenu.open(event, composeMenu(entries, menuEnv, "dashboard"));
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-y-auto" onContextMenu={openPageMenu}>
      <div className="mx-auto w-full max-w-[1440px] px-5 py-4">
        <section className="flex flex-wrap items-end justify-between gap-3">
          <div className="min-w-0">
            <p className="text-[11px] font-semibold uppercase tracking-[0.14em] text-brand-300">
              {t("dash.kicker")}
            </p>
            <h2 className="mt-1 text-[20px] font-semibold leading-tight text-text-strong">
              {t("dash.title")}
            </h2>
            <p className="mt-1 text-[11.5px] text-text-dim">{t("dash.subtitle")}</p>
          </div>
          <MetricStrip metrics={metrics} />
        </section>

        <div className="mt-4 grid min-w-0 gap-4 xl:grid-cols-[minmax(0,1fr)_280px]">
          <div className="flex min-w-0 flex-col gap-4">
            <section className="panel overflow-hidden">
              <header className="flex flex-wrap items-start justify-between gap-2 border-b border-line px-4 py-2.5">
                <div className="min-w-0">
                  <div className="flex items-center gap-2">
                    <h2 className="text-[13.5px] font-semibold text-text-strong">
                      {t("dash.graphTitle")}
                    </h2>
                    <span className="flex items-center gap-1 text-[11px] font-medium text-state-ok">
                      <span className="h-1.5 w-1.5 animate-pulse-soft rounded-full bg-state-ok" />
                      {t("dash.scanning")}
                    </span>
                  </div>
                  <p className="mt-0.5 truncate text-[11.5px] text-text-dim">
                    {t("dash.graphHint")} ·{" "}
                    {overview.report.generated_at
                      ? t("dash.scannedAt", { time: timeAgo(overview.report.generated_at) })
                      : t("dash.neverScanned")}
                  </p>
                </div>

                <div className="flex shrink-0 items-center gap-1.5">
                  <div
                    role="group"
                    aria-label={t("dash.filterGroup")}
                    className="flex items-center rounded-md border border-line bg-surface-2 p-[2px]"
                  >
                    <button
                      type="button"
                      onClick={() => setFocus("all")}
                      aria-pressed={focus === "all"}
                      className={clsx(
                        "cursor-pointer rounded px-2 py-[3px] text-[11.5px] transition-colors",
                        focus === "all"
                          ? "bg-ink-900 text-brand-300 shadow-panel"
                          : "text-text-dim hover:text-text-muted"
                      )}
                    >
                      {t("dash.all", { count: model.agents.length })}
                    </button>
                    <button
                      type="button"
                      onClick={() => setFocus("issues")}
                      aria-pressed={focus === "issues"}
                      disabled={brokenAgents === 0}
                      title={
                        brokenAgents === 0
                          ? t("dash.noBroken")
                          : t("dash.brokenAgents", { count: brokenAgents })
                      }
                      className={clsx(
                        "rounded px-2 py-[3px] text-[11.5px] transition-colors",
                        brokenAgents === 0
                          ? "cursor-not-allowed text-text-dim/60"
                          : focus === "issues"
                            ? "cursor-pointer bg-ink-900 text-state-danger shadow-panel"
                            : "cursor-pointer text-text-dim hover:text-text-muted"
                      )}
                    >
                      {t("dash.issues", { count: brokenAgents })}
                    </button>
                  </div>
                  {/*
                    用 stats.broken 而不是 duplicates 里的失效组：断链不一定构成重复组，
                    只看后者会出现「图谱标了异常 1，却没有修复入口」。
                  */}
                  {overview.report.stats.broken > 0 && (
                    <Button size="sm" variant="ghost" onClick={() => void planRepair()}>
                      <Wrench className="h-3.5 w-3.5" />
                      {t("dash.repairBroken")}
                    </Button>
                  )}
                </div>
              </header>

              <div className="relative h-[420px] bg-ink-950/40">
                <ResourceGraph model={model} focus={focus} onNodeContextMenu={openNodeMenu} />
              </div>
            </section>

            {/* 顶栏「N 处待处理」的落点：把体检项逐条摊开，每条都带动作 */}
            <PendingIssuesList onOpenSettings={onOpenSettings} />

            <DuplicateTable
              groups={visible}
              tab={tab}
              counts={counts}
              onTab={setTab}
              picked={picked}
              onToggle={toggle}
              onToggleAll={toggleAll}
              onSelect={onSelectGroup}
              onPlan={onPlan}
              totalWasted={totalWasted}
              onRowMenu={openRowMenu}
            />
          </div>

          <OverviewRail
            onOpenSkills={onOpenSkills}
            onOpenChanges={onOpenChanges}
            onReview={onPlan}
          />
        </div>
      </div>
    </div>
  );
}
