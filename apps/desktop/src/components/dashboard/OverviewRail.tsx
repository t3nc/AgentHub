import { useMemo, type ReactNode } from "react";
import {
  ArrowRight,
  FolderOpen,
  RotateCcw,
  ShieldCheck,
  Sparkles,
} from "lucide-react";
import { clsx } from "clsx";
import { AgentIcon } from "../agents/AgentIcon";
import { AgentHubMark } from "../brand/AgentHubMark";
import { Badge, Button, Dot } from "../ui";
import { useHub } from "../../lib/useHub";
import { useT } from "../../lib/i18n";
import { useSelection } from "../../lib/selection";
import { buildGraph, findAgent, findSkill } from "../../lib/graph";
import { PRIORITY_LABEL, priorityOf, suggestionsOf } from "../../lib/dashboard";
import { bytes, linkBadge, shortPath, timeAgo } from "../../lib/format";

function SectionTitle({ title, hint, aside }: { title: string; hint?: string; aside?: ReactNode }) {
  return (
    <div className="flex items-start justify-between gap-2">
      <div className="min-w-0">
        <h3 className="text-[12px] font-semibold text-text-strong">{title}</h3>
        {hint && <p className="mt-0.5 text-[11px] text-text-dim">{hint}</p>}
      </div>
      {aside}
    </div>
  );
}

/** 总览右栏：当前选择 / 建议处理 / 备份与回滚，只挂在本页，不占全局检查器的位置。 */
export function OverviewRail({
  onOpenSkills,
  onOpenChanges,
  onReview,
}: {
  onOpenSkills: (agentId?: string | null) => void;
  onOpenChanges: (batch?: string | null) => void;
  onReview: () => void;
}) {
  const { overview, rollback, planDedupe } = useHub();
  const t = useT();
  const { selection } = useSelection();

  const model = useMemo(() => (overview ? buildGraph(overview) : null), [overview]);
  const agent =
    selection?.kind === "agent" && model ? findAgent(model, selection.id) : null;
  const skill =
    selection?.kind === "skill" && overview ? findSkill(overview, selection.id) : null;

  const store = model?.store ?? null;
  const name =
    selection?.kind === "skill" && skill
      ? skill.name
      : agent
        ? agent.name
        : store
          ? t("common.store")
          : t("common.dash");
  const kindLabel =
    selection?.kind === "skill" && skill
      ? t("dash.rail.kind.skill")
      : agent
        ? t("dash.rail.kind.agent")
        : t("dash.rail.kind.store");
  const path =
    selection?.kind === "skill" && skill
      ? skill.path
      : agent
        ? agent.path
        : (store?.path ?? "");
  const link = skill ? linkBadge(skill.link) : null;
  const counts =
    selection?.kind === "skill" && skill
      ? t("dash.rail.single")
      : agent
        ? t("dash.rail.items", { count: agent.skillCount })
        : store
          ? t("dash.rail.masters", { count: store.skillCount })
          : t("common.dash");
  const healthTone =
    link?.tone === "danger"
      ? "danger"
      : agent?.health === "danger"
        ? "danger"
        : link?.tone === "warn" || agent?.health === "warn"
          ? "warn"
          : "ok";
  const healthText =
    link?.label ??
    (agent
      ? agent.broken > 0
        ? t("dash.rail.brokenLinks", { count: agent.broken })
        : agent.copies > 0
          ? t("dash.rail.copiesPending", { count: agent.copies })
          : t("dash.rail.healthy")
      : t("dash.rail.running"));

  const groups = overview?.report.duplicates ?? [];
  const suggestions = suggestionsOf(groups, 2);
  const lastBatch = overview?.batches[0] ?? null;

  return (
    <aside data-overview-rail className="panel hidden min-w-0 flex-col self-start xl:flex">
      <section className="border-b border-line px-3.5 py-3">
        <SectionTitle title={t("dash.rail.selection")} hint={t("dash.rail.selectionHint")} />
        <div className="mt-3 flex items-center gap-2.5">
          <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-line bg-surface-2 text-text-muted">
            {agent ? (
              <AgentIcon id={agent.id} className="h-4 w-4" />
            ) : skill ? (
              <AgentIcon id={skill.agent} className="h-4 w-4" />
            ) : (
              <AgentHubMark className="h-4 w-4 text-brand-300" />
            )}
          </span>
          <div className="min-w-0">
            <p className="truncate text-[12px] font-medium text-text-strong" title={name}>
              {name}
            </p>
            <p className="truncate text-[11.5px] text-text-dim">{kindLabel}</p>
          </div>
        </div>

        <div className="mt-2.5 flex items-start gap-1.5 rounded-md border border-line bg-ink-950/40 px-2 py-1">
          <FolderOpen className="mt-[2px] h-3 w-3 shrink-0 text-text-dim" />
          <code className="mono min-w-0 break-all text-[11px] leading-relaxed text-text-muted">
            {path ? shortPath(path, 4) : t("dash.rail.noPath")}
          </code>
        </div>

        <dl className="mt-2.5 space-y-1">
          <div className="flex items-center justify-between gap-2">
            <dt className="text-[11.5px] text-text-dim">{t("dash.rail.skillItems")}</dt>
            <dd className="font-mono text-[11px] text-text-muted">{counts}</dd>
          </div>
          <div className="flex items-center justify-between gap-2">
            <dt className="text-[11.5px] text-text-dim">{t("dash.rail.linkState")}</dt>
            <dd
              className={clsx(
                "flex items-center gap-1.5 text-[11px]",
                healthTone === "danger"
                  ? "text-state-danger"
                  : healthTone === "warn"
                    ? "text-state-warn"
                    : "text-state-ok"
              )}
            >
              <Dot tone={healthTone} />
              {healthText}
            </dd>
          </div>
        </dl>

        <button
          type="button"
          onClick={() => onOpenSkills(agent?.id ?? skill?.agent ?? null)}
          className="mt-2.5 flex cursor-pointer items-center gap-1 text-[11px] text-text-muted transition-colors hover:text-text-strong"
        >
          {t("dash.rail.viewSkills")} <ArrowRight className="h-3 w-3" />
        </button>
      </section>

      <section className="border-b border-line px-3.5 py-3">
        <SectionTitle
          title={t("dash.rail.suggest")}
          hint={t("dash.rail.suggestHint")}
          aside={
            <span className="rounded border border-state-warn/40 bg-state-warn/10 px-1.5 py-[1px] font-mono text-[11px] text-state-warn">
              {suggestions.length}
            </span>
          }
        />
        <div className="mt-2 space-y-1">
          {suggestions.length === 0 && (
            <p className="rounded-md border border-dashed border-line px-2 py-2 text-[11.5px] leading-relaxed text-text-dim">
              {t("dash.rail.noSuggest")}
            </p>
          )}
          {suggestions.map((group) => {
            const priority = priorityOf(group.wasted_bytes);
            return (
              <button
                key={group.name}
                type="button"
                onClick={() => void planDedupe([group.name])}
                title={t("dash.rail.planOne", { name: group.name })}
                className="flex w-full cursor-pointer items-center gap-2 rounded-md border border-transparent px-1.5 py-1.5 text-left transition-colors hover:border-line hover:bg-surface-hover"
              >
                <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-md border border-line bg-surface-2">
                  <Sparkles className="h-3.5 w-3.5 text-text-dim" />
                </span>
                <span className="min-w-0 flex-1">
                  <span className="block truncate text-[11.5px] font-medium text-text-strong">
                    {group.name}
                  </span>
                  <span className="block truncate text-[11px] text-text-dim">
                    {t("dash.rail.copiesWasted", {
                      copies: group.members.length,
                      bytes: bytes(group.wasted_bytes),
                    })}
                  </span>
                </span>
                <Badge tone={priority === "high" ? "warn" : priority === "medium" ? "info" : "dim"}>
                  {t(PRIORITY_LABEL[priority])}
                </Badge>
              </button>
            );
          })}
        </div>
        <Button size="sm" variant="outline" className="mt-2 w-full" onClick={onReview}>
          {t("dash.rail.review")} <ArrowRight className="h-3 w-3" />
        </Button>
      </section>

      <section className="px-3.5 py-3">
        <SectionTitle
          title={t("dash.rail.backup")}
          aside={
            <span className="flex items-center gap-1 text-[11px] text-state-ok">
              <Dot tone="ok" />
              {t("dash.rail.enabled")}
            </span>
          }
        />
        <div className="mt-2 flex items-start gap-2 rounded-md border border-line bg-ink-950/40 px-2 py-1.5">
          <ShieldCheck className="mt-[1px] h-3.5 w-3.5 shrink-0 text-state-ok" />
          <div className="min-w-0">
            <p className="text-[11px] text-text-muted">{t("dash.rail.autoBackup")}</p>
            <p className="mt-0.5 truncate text-[11px] text-text-dim">
              {lastBatch
                ? t("dash.rail.lastBatch", {
                    id: lastBatch.batch_id.slice(-6),
                    time: timeAgo(lastBatch.created_at),
                  })
                : t("dash.rail.noBatch")}
            </p>
          </div>
        </div>
        {lastBatch ? (
          <div className="mt-2 flex items-center gap-1.5">
            <Button
              size="sm"
              variant="outline"
              disabled={lastBatch.undone}
              className="flex-1"
              onClick={() => void rollback(lastBatch.batch_id)}
            >
              <RotateCcw className="h-3 w-3" />
              {lastBatch.undone ? t("common.rolledBack") : t("dash.rail.rollbackLatest")}
            </Button>
            <Button size="sm" variant="ghost" onClick={() => onOpenChanges(lastBatch.batch_id)}>
              {t("dash.rail.records")}
            </Button>
          </div>
        ) : (
          <button
            type="button"
            onClick={() => onOpenChanges(null)}
            className="mt-2 flex cursor-pointer items-center gap-1 text-[11px] text-text-muted transition-colors hover:text-text-strong"
          >
            {t("dash.rail.viewBatches")} <ArrowRight className="h-3 w-3" />
          </button>
        )}
      </section>

      <p className="flex items-start gap-1.5 border-t border-line px-3.5 py-2.5 text-[11px] leading-relaxed text-text-dim">
        <ShieldCheck className="mt-[1px] h-3 w-3 shrink-0" />
        {t("dash.rail.planNote")}
      </p>
    </aside>
  );
}
