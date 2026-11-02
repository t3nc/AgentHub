import { useEffect, useMemo, useState } from "react";
import { createPortal } from "react-dom";
import { clsx } from "clsx";
import {
  GitCompare,
  LayoutGrid,
  List,
  PackageOpen,
  ShieldQuestion,
  Trash2,
  Users,
  X,
} from "lucide-react";
import { useHub } from "../lib/useHub";
import { useT } from "../lib/i18n";
import type { MessageKey } from "../lib/i18n/messages";
import { useContextMenu, useMenuEnv } from "../lib/context-menu";
import { composeMenu } from "../lib/menu-builders/common";
import { skillPageMenu, skillRowMenu } from "../lib/menu-builders/skills";
import {
  agentInfo,
  bytes,
  linkBadge,
  shortHash,
  shortPath,
  strategyLabel,
} from "../lib/format";
import {
  junctionReason,
  LINK_STRATEGIES,
  isStrategyDisabled,
  junctionSupported,
} from "../lib/link-strategies";
import { AgentIcon } from "../components/agents/AgentIcon";
import { MarketDialog } from "../components/market/MarketDialog";
import { DeleteSkillsDialog } from "../components/skills/DeleteSkillsDialog";
import { Badge, Button, Empty, Panel, SearchInput, Toggle } from "../components/ui";
import type { LinkStrategyName, SkillRecord } from "../lib/types";

type SourceFilter = "all" | "store" | "agent_copy" | "readonly";

const FILTERS: { key: SourceFilter; label: MessageKey }[] = [
  { key: "all", label: "skills.source.all" },
  { key: "store", label: "skills.source.store" },
  { key: "agent_copy", label: "skills.source.agentCopy" },
  { key: "readonly", label: "skills.source.readonly" },
];

function SkillDetail({
  record,
  onClose,
  onCompare,
  onDelete,
  onContextMenu,
}: {
  record: SkillRecord;
  onClose: () => void;
  onCompare?: (name: string) => void;
  /** 删除这一条记录（只读项不可删） */
  onDelete?: (record: SkillRecord) => void;
  onContextMenu?: (event: React.MouseEvent) => void;
}) {
  const { overview, setSkillStrategy } = useHub();
  const t = useT();
  const [showTranslation, setShowTranslation] = useState(false);
  const badge = linkBadge(record.link);

  const strategies = overview?.strategies;
  const platformStrategy = strategies?.platform;
  const override = strategies?.overrides[record.name] ?? null;
  const effective = override ?? strategies?.default ?? null;

  return (
    <aside
      onContextMenu={onContextMenu}
      className="panel sticky top-0 flex max-h-[calc(100vh-120px)] w-[330px] shrink-0 flex-col"
    >
      <div className="flex items-start justify-between gap-2 border-b border-line px-3.5 py-3">
        <div className="min-w-0">
          <p className="truncate text-[13.5px] font-medium text-text-strong">{record.name}</p>
          <p className="mt-0.5 text-[11px] text-text-dim">{record.scope_label}</p>
        </div>
        <div className="flex shrink-0 items-center gap-1">
          {onCompare && (
            <button
              type="button"
              data-diff-entry={record.name}
              onClick={() => onCompare(record.name)}
              title={t("skills.compareTitle")}
              className="flex cursor-pointer items-center gap-1 rounded border border-line px-1.5 py-1 text-[11.5px] text-text-muted transition-colors hover:border-brand-500/40 hover:text-text-strong"
            >
              <GitCompare className="h-3 w-3" />
              {t("skills.compare")}
            </button>
          )}
          {onDelete && (
            <button
              type="button"
              data-delete-entry={record.name}
              disabled={record.readonly}
              onClick={() => onDelete(record)}
              title={
                record.readonly
                  ? t("skills.deleteReadonly")
                  : t("skills.deleteOneHint", { path: record.path })
              }
              className={clsx(
                "flex items-center gap-1 rounded border px-1.5 py-1 text-[11.5px] transition-colors",
                record.readonly
                  ? "cursor-not-allowed border-line text-text-dim opacity-40"
                  : "cursor-pointer border-line text-text-muted hover:border-state-danger/40 hover:text-state-danger"
              )}
            >
              <Trash2 className="h-3 w-3" />
              {t("skills.delete")}
            </button>
          )}
          <button
            type="button"
            onClick={onClose}
            className="cursor-pointer rounded p-1 text-text-dim hover:bg-surface-hover hover:text-text-strong"
            aria-label={t("skills.closeDetail")}
          >
            <X className="h-3.5 w-3.5" />
          </button>
        </div>
      </div>

      <div className="flex-1 space-y-3 overflow-y-auto px-3.5 py-3">
        <p className="text-[12px] leading-relaxed text-text-muted">
          {record.description ?? t("skills.noDescription")}
        </p>

        <div className="flex flex-wrap items-center gap-1">
          <Badge tone={badge.tone}>{badge.label}</Badge>
          <Badge tone="dim">
            {record.source === "store"
              ? t("skills.badge.store")
              : record.source === "readonly"
                ? t("skills.badge.readonly")
                : t("skills.badge.agentCopy")}
          </Badge>
          {record.agent_local && (
            <Badge tone="dim" title={t("skills.badge.localTitle")}>
              {t("skills.badge.local")}
            </Badge>
          )}
          {record.never_upload && (
            <Badge tone="warn">{t("skills.badge.neverUpload")}</Badge>
          )}
        </div>

        <dl className="space-y-1.5 text-[11.5px]">
          <div>
            <dt className="text-text-dim">{t("skills.field.path")}</dt>
            <dd className="mono break-all text-text-muted">{shortPath(record.path, 5)}</dd>
          </div>
          {record.link.kind === "symlink" || record.link.kind === "junction" || record.link.kind === "hardlink" ? (
            <div>
              <dt className="text-text-dim">{t("skills.field.target")}</dt>
              <dd className="mono break-all text-state-ok/90">{shortPath(record.link.target, 5)}</dd>
            </div>
          ) : null}
          <div className="flex gap-4">
            <div>
              <dt className="text-text-dim">{t("skills.field.size")}</dt>
              <dd className="mono text-text-muted">{bytes(record.size_bytes)}</dd>
            </div>
            <div>
              <dt className="text-text-dim">{t("skills.field.files")}</dt>
              <dd className="mono text-text-muted">{record.file_count}</dd>
            </div>
            <div>
              <dt className="text-text-dim">{t("skills.field.treeHash")}</dt>
              <dd className="mono text-text-muted">{shortHash(record.tree_hash)}</dd>
            </div>
          </div>
        </dl>

        <div className="rounded-lg border border-line bg-surface-2 p-2.5">
          <div className="flex items-center justify-between gap-2">
            <span className="text-[11.5px] text-text-muted">{t("skills.field.strategy")}</span>
            <select
              data-strategy-select={record.name}
              className="cursor-pointer rounded border border-line bg-ink-950 px-1.5 py-1 text-[11px] text-text-muted outline-none"
              value={override ?? "auto"}
              onChange={(e) => {
                const value = e.target.value;
                void setSkillStrategy(
                  record.name,
                  value === "auto" ? null : (value as LinkStrategyName)
                );
              }}
            >
              <option value="auto">
                {t("skills.followDefaultWith", {
                  strategy: strategyLabel(strategies?.default ?? "symlink"),
                })}
              </option>
              {LINK_STRATEGIES.map((item) => (
                <option
                  key={item.value}
                  value={item.value}
                  disabled={isStrategyDisabled(item.value, platformStrategy)}
                  title={
                    isStrategyDisabled(item.value, platformStrategy)
                      ? junctionReason()
                      : undefined
                  }
                >
                  {t(item.label)}
                  {isStrategyDisabled(item.value, platformStrategy)
                    ? t("common.strategy.onlyWindows")
                    : ""}
                </option>
              ))}
            </select>
          </div>
          <p
            data-strategy-effective={record.name}
            className="mt-1.5 text-[11.5px] leading-relaxed text-text-dim"
          >
            {t("skills.effective", { name: effective ? strategyLabel(effective) : "—" })}
            {override ? t("common.strategy.overridden") : t("common.strategy.followDefault")}
          </p>
          {!junctionSupported(platformStrategy) && (
            <p
              data-junction-unsupported
              className="mt-1 text-[11px] leading-relaxed text-text-dim"
            >
              {junctionReason()}
              {t("skills.platformDefault", {
                strategy: strategyLabel(platformStrategy ?? "symlink"),
              })}
            </p>
          )}
          <p className="mt-1 text-[11.5px] leading-relaxed text-text-dim">
            {t("skills.copyModeNote")}
          </p>
        </div>

        <div className="rounded-lg border border-line bg-surface-2 p-2.5">
          <button
            type="button"
            onClick={() => setShowTranslation((v) => !v)}
            className="flex w-full cursor-pointer items-center justify-between text-[11.5px] text-text-muted"
          >
            <span className="flex items-center gap-1.5">
              <ShieldQuestion className="h-3.5 w-3.5 text-text-dim" />
              {t("skills.translation.title")}
            </span>
            <Badge tone="dim">Phase 5</Badge>
          </button>
          {showTranslation && (
            <div className="mt-2 space-y-1.5 border-t border-line pt-2 text-[11.5px] leading-relaxed text-text-dim">
              <p>{t("skills.translation.disabledTitle")}</p>
              <p>{t("skills.translation.line1")}</p>
              <p>{t("skills.translation.line2")}</p>
              <p>{t("skills.translation.line3")}</p>
            </div>
          )}
        </div>
      </div>
    </aside>
  );
}

export function Skills({
  search,
  onSearch,
  initialAgent = null,
  onOpenDiff,
  onOpenTerminal,
}: {
  search: string;
  onSearch: (v: string) => void;
  /** 来自检查器的「只看这个 agent」，为 null 表示不预置筛选 */
  initialAgent?: string | null;
  /** 打开「同名技能版本对比」浮层（状态由 App 持有，检查器与命令面板共用） */
  onOpenDiff?: (name: string) => void;
  /** 在技能所在目录开终端（改 SKILL.md 不用自己 cd） */
  onOpenTerminal?: (cwd: string, name?: string) => void;
}) {
  const { overview, planDedupe, planDelete, setSkillStrategy, setAgentLocalSkill, notify } =
    useHub();
  const t = useT();
  const menuEnv = useMenuEnv();
  const contextMenu = useContextMenu();
  const [filter, setFilter] = useState<SourceFilter>("all");
  const [view, setView] = useState<"list" | "grid">("list");
  /** 选中按**路径**记：删除需要精确到「哪一条记录」，同一技能在多个 agent 下是不同记录 */
  const [selected, setSelected] = useState<string[]>([]);
  /** 待删清单：来自多选或详情面板的单条「删除」，非空即弹确认框 */
  const [deleteTargets, setDeleteTargets] = useState<SkillRecord[] | null>(null);
  const [marketOpen, setMarketOpen] = useState(false);
  const [detail, setDetail] = useState<SkillRecord | null>(null);
  const [hideLinked, setHideLinked] = useState(false);
  const [agents, setAgents] = useState<string[]>(initialAgent ? [initialAgent] : []);

  // 检查器再次指向另一个 agent 时，同步筛选（同值不重复设置，避免覆盖用户的多次选择）
  useEffect(() => {
    if (!initialAgent) return;
    setAgents((prev) => (prev.length === 1 && prev[0] === initialAgent ? prev : [initialAgent]));
  }, [initialAgent]);

  const records = overview?.report.records ?? [];

  const agentCounts = useMemo(() => {
    const map = new Map<string, number>();
    for (const record of records) {
      if (!record.agent) continue;
      map.set(record.agent, (map.get(record.agent) ?? 0) + 1);
    }
    return map;
  }, [records]);

  /** 已安装的 agent 在前，只出现在记录里的自定义平台追加在后 */
  const agentOptions = useMemo(() => {
    const ids = new Set<string>((overview?.report.agents ?? []).map((a) => a.id));
    for (const id of agentCounts.keys()) ids.add(id);
    return Array.from(ids).map((id) => ({ id, count: agentCounts.get(id) ?? 0 }));
  }, [overview, agentCounts]);

  const toggleAgent = (id: string) => {
    setAgents((prev) => (prev.includes(id) ? prev.filter((x) => x !== id) : [...prev, id]));
  };

  const visible = useMemo(() => {
    const keyword = search.trim().toLowerCase();
    return records
      .filter((r) => (filter === "all" ? true : r.source === filter))
      .filter((r) => (agents.length === 0 ? true : !!r.agent && agents.includes(r.agent)))
      .filter((r) => (hideLinked ? !(r.link.kind === "symlink" || r.link.kind === "junction") : true))
      .filter((r) =>
        keyword
          ? r.name.toLowerCase().includes(keyword) ||
            (r.description ?? "").toLowerCase().includes(keyword) ||
            r.path.toLowerCase().includes(keyword)
          : true
      )
      .sort((a, b) => a.name.localeCompare(b.name) || a.path.localeCompare(b.path));
  }, [records, filter, hideLinked, search, agents]);

  const duplicatedNames = new Set((overview?.report.duplicates ?? []).map((d) => d.name));

  const toggleSelect = (path: string) => {
    setSelected((prev) =>
      prev.includes(path) ? prev.filter((p) => p !== path) : [...prev, path]
    );
  };

  /** 选中项按当前扫描结果还原成记录（外部改动后消失的路径自动从选中里掉出去）。 */
  const selectedRecords = useMemo(
    () => records.filter((record) => selected.includes(record.path)),
    [records, selected]
  );
  /** 去重计划只吃技能名：同一技能勾了多份，也只传一个名字。 */
  const selectedNames = useMemo(
    () => [...new Set(selectedRecords.map((record) => record.name))],
    [selectedRecords]
  );
  const selectedBytes = selectedRecords.reduce((sum, record) => sum + record.size_bytes, 0);
  const storeRoot = overview?.report.store_root ?? "";

  const openRowMenu = (event: React.MouseEvent, record: SkillRecord) => {
    const siblings = records.filter((item) => item.name === record.name).length;
    contextMenu.open(
      event,
      composeMenu(
        skillRowMenu(record, {
          openDetail: () => setDetail(record),
          toggleSelect: () => toggleSelect(record.path),
          isSelected: selected.includes(record.path),
          hasSiblings: siblings > 1,
          effectiveStrategy: overview?.strategies?.overrides[record.name] ?? null,
          platformStrategy: overview?.strategies?.platform ?? null,
          setStrategy: (name, strategy) => void setSkillStrategy(name, strategy),
          toggleAgentLocal: (target) =>
            void setAgentLocalSkill(target.name, !target.agent_local),
          requestDelete: (target) => setDeleteTargets([target]),
          env: menuEnv,
          notify,
        }),
        menuEnv,
        "skills"
      )
    );
  };

  const openPageMenu = (event: React.MouseEvent) => {
    contextMenu.open(
      event,
      composeMenu(
        skillPageMenu({
          selectedCount: selectedRecords.length,
          planSelected: () => void planDedupe(selectedNames),
          clearSelection: () => setSelected([]),
          deleteSelected: () => setDeleteTargets(selectedRecords),
          openMarket: () => setMarketOpen(true),
          toggleLinkedOnly: () => setHideLinked((prev) => !prev),
          linkedOnly: hideLinked,
          toggleView: () => setView((prev) => (prev === "list" ? "grid" : "list")),
          view,
        }),
        menuEnv,
        "skills"
      )
    );
  };

  return (
    <div className="flex gap-3.5 animate-fade-up" onContextMenu={openPageMenu}>
      <div className="min-w-0 flex-1 space-y-3">
        <div className="panel flex flex-wrap items-center gap-2 px-3 py-2.5">
          <div className="w-[220px]">
            <SearchInput
              value={search}
              onChange={onSearch}
              placeholder={t("skills.searchPlaceholder")}
            />
          </div>
          <div className="flex items-center gap-1">
            {FILTERS.map((f) => (
              <button
                key={f.key}
                type="button"
                onClick={() => setFilter(f.key)}
                className={clsx(
                  "cursor-pointer rounded-md px-2 py-1 text-[11.5px] transition-colors",
                  filter === f.key
                    ? "bg-brand-500/15 text-brand-300"
                    : "text-text-muted hover:bg-surface-2 hover:text-text-strong"
                )}
              >
                {t(f.label)}
              </button>
            ))}
          </div>
          <label className="ml-2 flex cursor-pointer items-center gap-1.5 text-[11.5px] text-text-muted">
            <Toggle checked={hideLinked} onChange={setHideLinked} />
            {t("skills.onlyUnadopted")}
          </label>
          <div className="ml-auto flex items-center gap-2">
            <Button variant="outline" size="sm" onClick={() => setMarketOpen(true)}>
              <PackageOpen className="h-3.5 w-3.5" />
              {t("skills.installFromMarket")}
            </Button>
            <div className="flex items-center gap-1 rounded-md border border-line p-0.5">
              <button
                type="button"
                onClick={() => setView("list")}
                className={clsx(
                  "cursor-pointer rounded p-1 transition-colors",
                  view === "list" ? "bg-surface-active text-text-strong" : "text-text-dim"
                )}
                aria-label={t("skills.view.list")}
              >
                <List className="h-3.5 w-3.5" />
              </button>
              <button
                type="button"
                onClick={() => setView("grid")}
                className={clsx(
                  "cursor-pointer rounded p-1 transition-colors",
                  view === "grid" ? "bg-surface-active text-text-strong" : "text-text-dim"
                )}
                aria-label={t("skills.view.grid")}
              >
                <LayoutGrid className="h-3.5 w-3.5" />
              </button>
            </div>
          </div>

          <div className="flex w-full flex-wrap items-center gap-1.5 border-t border-line pt-2">
            <span className="mr-1 flex items-center gap-1 text-[11px] text-text-dim">
              <Users className="h-3 w-3" />
              {t("skills.filterByAgent")}
            </span>
            <button
              type="button"
              onClick={() => setAgents([])}
              className={clsx(
                "cursor-pointer rounded-md border px-2 py-[3px] text-[11px] transition-colors",
                agents.length === 0
                  ? "border-brand-500/40 bg-brand-500/15 text-brand-300"
                  : "border-line text-text-muted hover:bg-surface-2 hover:text-text-strong"
              )}
            >
              {t("skills.allAgents")}
            </button>
            {agentOptions.map(({ id, count }) => {
              const info = agentInfo(id);
              const active = agents.includes(id);
              const disabled = count === 0;
              return (
                <button
                  key={id}
                  type="button"
                  data-agent={id}
                  aria-pressed={active}
                  disabled={disabled}
                  onClick={() => toggleAgent(id)}
                  title={
                    disabled
                      ? t("skills.agentEmpty", { name: info.name })
                      : t("skills.agentCount", { name: info.name, count })
                  }
                  className={clsx(
                    "flex items-center gap-1.5 rounded-md border px-2 py-[3px] text-[11px] transition-colors",
                    disabled
                      ? "cursor-not-allowed border-line text-text-dim opacity-45"
                      : active
                        ? "cursor-pointer border-brand-500/40 bg-brand-500/15 text-brand-300"
                        : "cursor-pointer border-line text-text-muted hover:bg-surface-2 hover:text-text-strong"
                  )}
                >
                  <span className={clsx("flex items-center rounded border px-1", info.accent)}>
                    <AgentIcon id={id} className="h-3.5 w-3.5" />
                  </span>
                  {info.name}
                  <span className="mono text-[11px] text-text-dim">{count}</span>
                </button>
              );
            })}
            {agents.length > 0 && (
              <button
                type="button"
                onClick={() => setAgents([])}
                className="ml-1 cursor-pointer rounded px-1.5 py-[3px] text-[11.5px] text-text-dim transition-colors hover:bg-surface-2 hover:text-text-strong"
              >
                {t("skills.clearFilter")}
              </button>
            )}
            <span className="mono ml-auto text-[11.5px] text-text-dim">
              {visible.length} / {records.length}
            </span>
          </div>
        </div>

        {visible.length === 0 ? (
          <Panel>
            <Empty
              title={t("skills.emptyTitle")}
              hint={
                agents.length > 0
                  ? t("skills.emptyAgentHint", {
                      agents: agents.map((id) => agentInfo(id).name).join(" / "),
                    })
                  : t("skills.emptyHint")
              }
            />
          </Panel>
        ) : view === "list" ? (
          <Panel bodyClassName="p-0">
            <table className="w-full text-[12px]">
              <thead>
                <tr className="border-b border-line text-left text-[11px] text-text-dim">
                  <th className="w-8 py-2 pl-3" />
                  <th className="py-2 pr-2 font-normal">{t("skills.col.skill")}</th>
                  <th className="w-[112px] py-2 font-normal">agent</th>
                  <th className="w-[90px] py-2 font-normal">{t("skills.col.status")}</th>
                  <th className="w-[76px] py-2 text-right font-normal">{t("skills.col.size")}</th>
                  <th className="w-[84px] py-2 pr-3 text-right font-normal">
                    {t("skills.col.treeHash")}
                  </th>
                </tr>
              </thead>
              <tbody>
                {visible.map((record) => {
                  const badge = linkBadge(record.link);
                  const info = agentInfo(record.agent);
                  return (
                    <tr
                      key={`${record.path}`}
                      onClick={() => setDetail(record)}
                      onContextMenu={(event) => openRowMenu(event, record)}
                      data-ctx="skill-row"
                      className={clsx(
                        "cursor-pointer border-b border-line transition-colors hover:bg-surface-2",
                        detail?.path === record.path && "bg-surface-hover"
                      )}
                    >
                      <td className="py-1.5 pl-2">
                        <input
                          type="checkbox"
                          checked={selected.includes(record.path)}
                          disabled={record.readonly}
                          onChange={() => toggleSelect(record.path)}
                          onClick={(event) => event.stopPropagation()}
                          title={
                            record.readonly
                              ? t("skills.deleteReadonly")
                              : t("skills.select", { name: record.name })
                          }
                          className={clsx(
                            "h-3 w-3 accent-[rgb(var(--brand-500))]",
                            record.readonly
                              ? "cursor-not-allowed opacity-40"
                              : "cursor-pointer"
                          )}
                          aria-label={t("skills.select", { name: record.name })}
                        />
                      </td>
                      <td className="max-w-0 py-1.5 pr-2">
                        <div className="flex items-center gap-1.5">
                          <span className="truncate text-text-strong">{record.name}</span>
                          {duplicatedNames.has(record.name) && (
                            <Badge tone="warn">{t("skills.badge.duplicate")}</Badge>
                          )}
                          {record.readonly && (
                            <Badge tone="dim">{t("skills.badge.readonly")}</Badge>
                          )}
                          {record.agent_local && (
                            <Badge tone="dim" title={t("skills.badge.localShortTitle")}>
                              {t("skills.badge.local")}
                            </Badge>
                          )}
                          {onOpenDiff && duplicatedNames.has(record.name) && (
                            <button
                              type="button"
                              data-diff-row={record.name}
                              onClick={(event) => {
                                event.stopPropagation();
                                onOpenDiff(record.name);
                              }}
                              aria-label={t("skills.compareAria", { name: record.name })}
                              className="hidden shrink-0 cursor-pointer rounded border border-line px-1 text-[11px] text-text-dim transition-colors hover:border-brand-500/40 hover:text-text-strong xl:inline-block"
                            >
                              {t("skills.compare")}
                            </button>
                          )}
                          {onOpenTerminal && (
                            <button
                              type="button"
                              onClick={(event) => {
                                event.stopPropagation();
                                // path 就是技能目录；万一给的是 SKILL.md，退到父目录
                                const cwd = record.path.replace(/[\\/][^\\/]+\.md$/i, "");
                                onOpenTerminal(cwd, record.name);
                              }}
                              title={t("skills.terminalHereTitle", { path: record.path })}
                              aria-label={t("skills.terminalHereAria", { name: record.name })}
                              className="hidden shrink-0 cursor-pointer rounded border border-line px-1 text-[11px] text-text-dim transition-colors hover:border-brand-500/40 hover:text-text-strong xl:inline-block"
                            >
                              {t("skills.terminal")}
                            </button>
                          )}
                        </div>
                        <p className="truncate text-[11px] text-text-dim">
                          {record.description ?? shortPath(record.path, 3)}
                        </p>
                      </td>
                      <td className="py-1.5">
                        <span
                          className={clsx(
                            "inline-flex items-center rounded border px-1.5 py-[1px]",
                            info.accent
                          )}
                        >
                          <AgentIcon id={record.agent} className="h-3.5 w-3.5" />
                        </span>
                      </td>
                      <td className="py-1.5">
                        <Badge tone={badge.tone}>{badge.label}</Badge>
                      </td>
                      <td className="mono py-1.5 text-right text-text-muted">
                        {bytes(record.size_bytes)}
                      </td>
                      <td className="mono py-1.5 pr-3 text-right text-text-dim">
                        {shortHash(record.tree_hash)}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </Panel>
        ) : (
          <div className="grid grid-cols-3 gap-2.5">
            {visible.map((record) => {
              const badge = linkBadge(record.link);
              const picked = selected.includes(record.path);
              return (
                <div
                  key={record.path}
                  onContextMenu={(event) => openRowMenu(event, record)}
                  data-ctx="skill-card"
                  className={clsx(
                    "panel hover-lift relative px-3 py-2.5",
                    picked && "border-brand-500/50 shadow-glow"
                  )}
                >
                  <div className="flex items-center justify-between gap-2">
                    <button
                      type="button"
                      onClick={() => setDetail(record)}
                      className="min-w-0 cursor-pointer truncate text-left text-[12.5px] text-text-strong"
                    >
                      {record.name}
                    </button>
                    <div className="flex shrink-0 items-center gap-1.5">
                      <span
                        className={clsx(
                          "inline-flex items-center rounded border px-1 py-[1px]",
                          agentInfo(record.agent).accent
                        )}
                      >
                        <AgentIcon id={record.agent} className="h-3.5 w-3.5" />
                      </span>
                      <input
                        type="checkbox"
                        checked={picked}
                        disabled={record.readonly}
                        onChange={() => toggleSelect(record.path)}
                        title={
                          record.readonly
                            ? t("skills.deleteReadonly")
                            : t("skills.select", { name: record.name })
                        }
                        aria-label={t("skills.select", { name: record.name })}
                        className={clsx(
                          "h-3 w-3 accent-[rgb(var(--brand-500))]",
                          record.readonly ? "cursor-not-allowed opacity-40" : "cursor-pointer"
                        )}
                      />
                    </div>
                  </div>
                  <button
                    type="button"
                    onClick={() => setDetail(record)}
                    className="mt-1 block w-full cursor-pointer text-left"
                  >
                    <p className="line-clamp-2 text-[11px] leading-relaxed text-text-dim">
                      {record.description ?? "—"}
                    </p>
                    <div className="mt-2 flex items-center justify-between">
                      <span className="flex items-center gap-1">
                        <Badge tone={badge.tone}>{badge.label}</Badge>
                        {record.agent_local && (
                          <Badge tone="dim" title={t("skills.badge.localShortTitle")}>
                            {t("skills.badge.local")}
                          </Badge>
                        )}
                      </span>
                      <span className="mono text-[11.5px] text-text-dim">
                        {bytes(record.size_bytes)}
                      </span>
                    </div>
                  </button>
                </div>
              );
            })}
          </div>
        )}
      </div>

      {detail ? (
        <SkillDetail
          record={detail}
          onClose={() => setDetail(null)}
          onCompare={onOpenDiff}
          onDelete={(record) => setDeleteTargets([record])}
          onContextMenu={(event) => openRowMenu(event, detail)}
        />
      ) : (
        <div className="hidden w-[1px] xl:block" />
      )}

      {/* 挂到 body：页面外层用了带动画的容器，`position: fixed` 会被它的 transform 收编 */}
      {selectedRecords.length > 0 &&
        createPortal(
          <div className="fixed bottom-[42px] left-[260px] z-30 flex items-center gap-3 rounded-xl border border-brand-500/30 bg-ink-900/95 px-3.5 py-2 shadow-glow backdrop-blur-xl">
            <Badge tone="brand">
              {t("skills.selectedCount", { count: selectedRecords.length })}
            </Badge>
            <span className="text-[11.5px] text-text-muted">
              {t("skills.willFree")}{" "}
              <span className="mono text-text-strong">{bytes(selectedBytes)}</span>
            </span>
            <Button variant="primary" size="sm" onClick={() => void planDedupe(selectedNames)}>
              {t("skills.planSelected")}
            </Button>
            <Button variant="danger" size="sm" onClick={() => setDeleteTargets(selectedRecords)}>
              <Trash2 className="h-3 w-3" />
              {t("skills.delete")}
            </Button>
            <Button variant="ghost" size="sm" onClick={() => setSelected([])}>
              {t("common.cancel")}
            </Button>
          </div>,
          document.body
        )}

      {marketOpen && <MarketDialog onClose={() => setMarketOpen(false)} />}

      {deleteTargets && deleteTargets.length > 0 && (
        <DeleteSkillsDialog
          records={deleteTargets}
          allRecords={records}
          storeRoot={storeRoot}
          onCancel={() => setDeleteTargets(null)}
          onConfirm={(opts) => {
            const paths = deleteTargets.map((record) => record.path);
            setDeleteTargets(null);
            void planDelete(paths, opts);
          }}
        />
      )}
    </div>
  );
}
