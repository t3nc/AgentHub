import { ArrowRight, Boxes, Link2 } from "lucide-react";
import { clsx } from "clsx";
import { AgentIcon } from "../agents/AgentIcon";
import { Badge, Button, Empty } from "../ui";
import {
  PRIORITY_LABEL,
  priorityOf,
  representativeOf,
  TAB_LABEL,
  type DuplicateTab,
  type TabCounts,
} from "../../lib/dashboard";
import { bytes, tailPath } from "../../lib/format";
import { useT } from "../../lib/i18n";
import type { DuplicateGroup } from "../../lib/types";

const TABS: DuplicateTab[] = ["optimize", "all", "broken"];

const PRIORITY_TONE = { high: "warn", medium: "info", low: "dim" } as const;

function sourcesOf(group: DuplicateGroup): string[] {
  return Array.from(new Set(group.members.map((member) => member.agent ?? "store")));
}

export function DuplicateTable({
  groups,
  tab,
  counts,
  onTab,
  picked,
  onToggle,
  onToggleAll,
  onSelect,
  onPlan,
  totalWasted,
  onRowMenu,
}: {
  groups: DuplicateGroup[];
  tab: DuplicateTab;
  counts: TabCounts;
  onTab: (tab: DuplicateTab) => void;
  picked: string[];
  onToggle: (name: string) => void;
  onToggleAll: () => void;
  onSelect: (group: DuplicateGroup) => void;
  onPlan: () => void;
  totalWasted: number;
  /** 行右键：交给总览页弹菜单 */
  onRowMenu?: (event: React.MouseEvent, group: DuplicateGroup) => void;
}) {
  const t = useT();
  const allPicked = groups.length > 0 && picked.length === groups.length;

  return (
    <section data-result-panel className="panel flex min-w-0 flex-col">
      <header className="flex flex-wrap items-start justify-between gap-3 border-b border-line px-4 py-3">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <h2 className="text-[13.5px] font-semibold text-text-strong">
              {t("dash.dup.title")}
            </h2>
            <span className="rounded border border-line bg-surface-2 px-1.5 py-[1px] font-mono text-[11px] text-text-muted">
              {t("dash.dup.groups", { count: counts.all })}
            </span>
          </div>
          <p className="mt-1 text-[11px] text-text-dim">{t("dash.dup.hint")}</p>
        </div>
        {groups.length > 0 && (
          <button
            type="button"
            onClick={onToggleAll}
            className="cursor-pointer text-[11px] text-text-muted transition-colors hover:text-text-strong"
          >
            {allPicked ? t("dash.dup.clearSelection") : t("dash.dup.selectAll")}
          </button>
        )}
      </header>

      <div className="flex items-center gap-1 border-b border-line px-3">
        {TABS.map((key) => (
          <button
            key={key}
            type="button"
            role="tab"
            aria-selected={tab === key}
            onClick={() => onTab(key)}
            className={clsx(
              "relative cursor-pointer px-2.5 py-2 text-[11.5px] transition-colors",
              tab === key
                ? "text-text-strong"
                : "text-text-dim hover:text-text-muted"
            )}
          >
            {t(TAB_LABEL[key])}{" "}
            <span className="font-mono text-[11px] text-text-dim">{counts[key]}</span>
            {tab === key && (
              <span className="absolute inset-x-1.5 -bottom-px h-[2px] rounded-full bg-brand-500" />
            )}
          </button>
        ))}
      </div>

      <div className="min-w-0 overflow-x-auto">
        {groups.length === 0 ? (
          <Empty
            title={
              tab === "broken" ? t("dash.dup.emptyBroken") : t("dash.dup.emptyOptimize")
            }
            hint={
              tab === "broken"
                ? t("dash.dup.emptyBrokenHint")
                : t("dash.dup.emptyOptimizeHint")
            }
          />
        ) : (
          <table className="w-full min-w-[640px] border-collapse text-[11.5px]">
            <thead>
              <tr className="border-b border-line text-left text-[11px] uppercase tracking-wider text-text-dim">
                <th scope="col" className="w-9 py-1.5 pl-3">
                  <span className="sr-only">{t("dash.dup.select")}</span>
                </th>
                <th scope="col" className="py-1.5 pr-2 font-normal">
                  {t("dash.dup.col.skill")}
                </th>
                <th scope="col" className="w-[132px] py-1.5 pr-2 font-normal">
                  {t("dash.dup.col.source")}
                </th>
                <th scope="col" className="w-[64px] py-1.5 pr-2 font-normal">
                  {t("dash.dup.col.copies")}
                </th>
                <th scope="col" className="hidden w-[90px] py-1.5 pr-2 font-normal md:table-cell">
                  {t("dash.dup.col.wasted")}
                </th>
                <th scope="col" className="w-[104px] py-1.5 pr-2 font-normal">
                  {t("dash.dup.col.status")}
                </th>
                <th scope="col" className="w-[64px] py-1.5 pr-3 font-normal">
                  <span className="sr-only">{t("dash.dup.col.actions")}</span>
                </th>
              </tr>
            </thead>
            <tbody>
              {groups.map((group) => {
                const checked = picked.includes(group.name);
                const priority = priorityOf(group.wasted_bytes);
                const brokenCount = group.members.filter(
                  (member) => member.link.kind === "broken"
                ).length;
                const sample = representativeOf(group);
                return (
                  <tr
                    key={group.name}
                    data-ctx="dup-row"
                    onContextMenu={
                      onRowMenu ? (event) => onRowMenu(event, group) : undefined
                    }
                    className={clsx(
                      "border-b border-line/70 transition-colors",
                      checked ? "bg-brand-500/[0.06]" : "hover:bg-surface-hover"
                    )}
                  >
                    <td className="py-1.5 pl-3 align-top">
                      <input
                        type="checkbox"
                        checked={checked}
                        onChange={() => onToggle(group.name)}
                        aria-label={t("dash.dup.selectOne", { name: group.name })}
                        className="mt-[3px] h-3.5 w-3.5 cursor-pointer accent-[rgb(var(--brand-500))]"
                      />
                    </td>
                    <td className="min-w-0 py-1.5 pr-2">
                      <button
                        type="button"
                        onClick={() => onSelect(group)}
                        className="block w-full cursor-pointer text-left"
                        title={group.members.map((member) => member.path).join("\n")}
                      >
                        <span className="block truncate font-medium text-text-strong">
                          {group.name}
                        </span>
                        <span className="mono block truncate text-[11.5px] text-text-dim">
                          {tailPath(sample?.path ?? "", 3)}
                        </span>
                      </button>
                    </td>
                    <td className="py-1.5 pr-2">
                      <span className="flex items-center gap-1">
                        {sourcesOf(group).map((id) => (
                          <span
                            key={id}
                            title={id === "store" ? t("common.store") : id}
                            className="flex"
                          >
                            <AgentIcon id={id === "store" ? null : id} className="h-3.5 w-3.5" />
                          </span>
                        ))}
                      </span>
                    </td>
                    <td className="py-1.5 pr-2 font-mono text-text-muted">
                      {t("dash.dup.copies", { count: group.members.length })}
                    </td>
                    <td className="hidden py-1.5 pr-2 font-mono text-state-warn md:table-cell">
                      {bytes(group.wasted_bytes)}
                    </td>
                    <td className="py-1.5 pr-2">
                      <span className="flex items-center gap-1.5">
                        <Badge tone={brokenCount > 0 ? "danger" : group.identical ? "ok" : "warn"}>
                          {brokenCount > 0
                            ? t("dash.dup.brokenCount", { count: brokenCount })
                            : group.identical
                              ? t("dash.dup.identical")
                              : t("dash.dup.differs")}
                        </Badge>
                        <span
                          className={clsx(
                            "font-mono text-[11px]",
                            priority === "high"
                              ? "text-state-warn"
                              : priority === "medium"
                                ? "text-state-info"
                                : "text-text-dim"
                          )}
                          title={t("dash.dup.priorityHint")}
                        >
                          {t(PRIORITY_LABEL[priority])}
                        </span>
                      </span>
                    </td>
                    <td className="py-1.5 pr-3 text-right">
                      <button
                        type="button"
                        onClick={() => onSelect(group)}
                        className="cursor-pointer rounded border border-transparent px-1.5 py-0.5 text-[11.5px] text-text-muted transition-colors hover:border-line-strong hover:text-text-strong"
                      >
                        {t("dash.dup.review")}
                      </button>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}
      </div>

      <footer className="flex flex-wrap items-center gap-x-3 gap-y-1.5 border-t border-line px-4 py-2.5">
        <span data-selection-summary className="text-[11px] text-text-dim">
          {t("dash.dup.summary", {
            picked: picked.length,
            shown: groups.length,
            bytes: bytes(totalWasted),
          })}
        </span>
        <Button
          size="sm"
          variant="primary"
          className="ml-auto"
          disabled={counts.optimize === 0 && counts.broken === 0}
          onClick={onPlan}
        >
          <Boxes className="h-3.5 w-3.5" />
          {t("dash.dup.plan")}
          <ArrowRight className="h-3.5 w-3.5" />
        </Button>
      </footer>

      {picked.length > 0 && (
        <div className="flex items-center gap-3 border-t border-line bg-ink-850 px-4 py-2">
          <span className="flex items-center gap-1.5 text-[11.5px] text-text-muted">
            <Link2 className="h-3.5 w-3.5 text-brand-300" />
            {t("dash.dup.pickedGroups", { count: picked.length })}
          </span>
          <Button size="sm" variant="outline" className="ml-auto" onClick={onPlan}>
            {t("dash.dup.reviewPicked")}
          </Button>
        </div>
      )}
    </section>
  );
}
