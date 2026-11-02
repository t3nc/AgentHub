import { useMemo, type ReactNode } from "react";
import { ArrowRight, Bot, Boxes, GitCompare, X } from "lucide-react";
import { Badge, Button } from "../ui";
import type { NavKey } from "../../lib/nav";
import { useHub } from "../../lib/useHub";
import { useT } from "../../lib/i18n";
import { useSelection } from "../../lib/selection";
import type { LinkStrategyName } from "../../lib/types";
import { agentInfo, bytes, linkBadge, shortPath, strategyLabel, timeAgo } from "../../lib/format";
import {
  LINK_STRATEGIES,
  isStrategyDisabled,
  junctionReason,
  junctionSupported,
} from "../../lib/link-strategies";
import { buildGraph, findAgent, findSkill } from "../../lib/graph";

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex items-start justify-between gap-3 py-1">
      <span className="shrink-0 text-[11px] text-text-dim">{label}</span>
      <span className="min-w-0 text-right text-[11.5px] text-text-muted">{children}</span>
    </div>
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="border-t border-line px-3 py-2.5 first:border-t-0">
      <p className="mb-1 text-[11px] uppercase tracking-wider text-text-dim">{title}</p>
      {children}
    </section>
  );
}

export function Inspector({
  onNavigate,
  onInspectAgent,
  onOpenDiff,
  onOpenAgentEnv,
  onClose,
}: {
  onNavigate: (key: NavKey) => void;
  onInspectAgent: (agentId: string) => void;
  onOpenDiff: (name: string) => void;
  /** 跳到「Agent 环境」并定位到该内置平台对应的产品行 */
  onOpenAgentEnv: (agentId: string) => void;
  onClose: () => void;
}) {
  const { overview, planRepair, planDedupe, setSkillStrategy, refresh } = useHub();
  const { selection, clear } = useSelection();
  const t = useT();

  const model = useMemo(() => (overview ? buildGraph(overview) : null), [overview]);
  const agent =
    selection && selection.kind === "agent" && model ? findAgent(model, selection.id) : null;
  const skill =
    selection && selection.kind === "skill" && overview
      ? findSkill(overview, selection.id)
      : null;

  const title =
    selection?.kind === "store"
      ? t("common.store")
      : agent
        ? agent.name
        : skill
          ? skill.name
          : t("inspector.title");
  const subtitle =
    selection?.kind === "store"
      ? t("inspector.subtitle.store")
      : agent
        ? t("inspector.subtitle.agent")
        : skill
          ? t("inspector.subtitle.skill")
          : t("inspector.subtitle.empty");

  return (
    <aside
      data-inspector
      className="flex h-full w-[320px] shrink-0 flex-col border-l border-line bg-ink-900"
    >
      <header className="flex h-12 shrink-0 items-center gap-1.5 border-b border-line px-3">
        <div className="min-w-0 flex-1">
          <p className="truncate text-[12.5px] font-medium text-text-strong">{title}</p>
          <p className="truncate text-[11px] text-text-dim">{subtitle}</p>
        </div>
        {selection && (
          <button
            type="button"
            onClick={clear}
            aria-label={t("inspector.clearSelection")}
            className="cursor-pointer rounded p-1 text-text-dim transition-colors hover:bg-surface-hover hover:text-text-strong"
          >
            <X className="h-3.5 w-3.5" />
          </button>
        )}
        <button
          type="button"
          onClick={onClose}
          aria-label={t("inspector.collapse")}
          title={t("inspector.collapse")}
          className="cursor-pointer rounded p-1 text-text-dim transition-colors hover:bg-surface-hover hover:text-text-strong"
        >
          <ArrowRight className="h-3.5 w-3.5" />
        </button>
      </header>

      <div className="flex-1 overflow-y-auto">
        {!overview || !model ? (
          <p className="px-3 py-4 text-[11.5px] text-text-dim">{t("inspector.loading")}</p>
        ) : selection?.kind === "store" ? (
          <>
            <Section title={t("inspector.section.store")}>
              <Row label={t("inspector.path")}>
                <span className="mono break-all text-[11.5px]" title={model.store.path}>
                  {shortPath(model.store.path, 3)}
                </span>
              </Row>
              <Row label={t("inspector.skillCount")}>{model.store.skillCount}</Row>
              <Row label={t("inspector.size")}>{bytes(model.store.sizeBytes)}</Row>
              <Row label={t("inspector.shared")}>{model.store.sharedSkills}</Row>
            </Section>
            <Section title={t("inspector.section.actions")}>
              <div className="flex flex-col gap-1.5">
                <Button size="sm" variant="outline" onClick={() => void planDedupe()}>
                  {t("inspector.planDedupe")}
                </Button>
                <Button size="sm" variant="ghost" onClick={() => onNavigate("skills")}>
                  {t("inspector.openLibrary")}
                </Button>
              </div>
            </Section>
          </>
        ) : agent ? (
          <>
            <Section title={t("inspector.section.dir")}>
              <Row label={t("inspector.configDir")}>
                <span className="mono break-all text-[11.5px]">{shortPath(agent.path, 2)}</span>
              </Row>
              <Row label={t("inspector.cli")}>
                <span className="mono text-[11.5px]">
                  {agent.cliPath ?? t("inspector.notDetected")}
                </span>
              </Row>
              <Row label={t("inspector.status")}>
                <Badge tone={agent.detected ? "ok" : "dim"}>
                  {agent.detected ? t("inspector.installed") : t("inspector.notDetected")}
                </Badge>
              </Row>
            </Section>
            <Section title={t("inspector.section.distribution")}>
              <Row label={t("inspector.skillCount")}>{agent.skillCount}</Row>
              <Row label={t("inspector.linked")}>
                <span className="text-state-ok">{agent.linked}</span>
              </Row>
              <Row label={t("inspector.copies")}>
                <span className={agent.copies > 0 ? "text-state-warn" : ""}>{agent.copies}</span>
              </Row>
              <Row label={t("inspector.broken")}>
                <span className={agent.broken > 0 ? "text-state-danger" : ""}>{agent.broken}</span>
              </Row>
              <Row label={t("inspector.foreign")}>{agent.foreign}</Row>
              <Row label={t("inspector.readonly")}>{agent.readonly}</Row>
              <Row label={t("inspector.size")}>{bytes(agent.sizeBytes)}</Row>
            </Section>
            <Section title={t("inspector.section.capabilities")}>
              <div className="flex flex-wrap gap-1">
                {Object.entries(agent.capabilities).map(([key, enabled]) => (
                  <Badge key={key} tone={enabled ? "brand" : "dim"}>
                    {key}
                  </Badge>
                ))}
              </div>
            </Section>
            <Section title={t("inspector.section.actions")}>
              <div className="flex flex-col gap-1.5">
                <Button size="sm" variant="outline" onClick={() => onInspectAgent(agent.id)}>
                  {t("inspector.filterInLibrary")}
                </Button>
                <Button size="sm" variant="outline" onClick={() => onOpenAgentEnv(agent.id)}>
                  <Bot className="h-3 w-3" />
                  {t("inspector.installGuide")}
                </Button>
                {agent.broken > 0 && (
                  <Button size="sm" variant="outline" onClick={() => void planRepair()}>
                    {t("inspector.planRepairCount", { count: agent.broken })}
                  </Button>
                )}
                {agent.copies > 0 && (
                  <Button size="sm" variant="outline" onClick={() => void planDedupe()}>
                    {t("inspector.planDedupe")}
                  </Button>
                )}
              </div>
            </Section>
          </>
        ) : skill ? (
          <>
            <Section title={t("inspector.section.skill")}>
              <Row label={t("inspector.source")}>
                <Badge tone={skill.agent ? "dim" : "brand"}>
                  {skill.agent ? agentInfo(skill.agent).name : t("common.store")}
                </Badge>
              </Row>
              <Row label={t("inspector.path")}>
                <span className="mono break-all text-[11.5px]" title={skill.path}>
                  {shortPath(skill.path, 3)}
                </span>
              </Row>
              <Row label={t("inspector.link")}>{linkBadge(skill.link).label}</Row>
              <Row label={t("inspector.sizeLabel")}>{bytes(skill.size_bytes)}</Row>
              <Row label={t("inspector.files")}>{skill.file_count}</Row>
              <Row label={t("inspector.modified")}>{timeAgo(skill.modified_at)}</Row>
              <Row label={t("inspector.treeHash")}>
                <span className="mono text-[11px]">
                  {skill.tree_hash?.slice(0, 10) ?? t("common.dash")}
                </span>
              </Row>
              {skill.readonly && (
                <Row label={t("inspector.readonly")}>
                  <Badge tone="warn">{t("inspector.builtin")}</Badge>
                </Row>
              )}
            </Section>

            {skill.agent && (
              <Section title={t("inspector.section.strategy")}>
                <select
                  data-inspector-strategy
                  value={overview.strategies.overrides[skill.name] ?? "auto"}
                  onChange={(event) => {
                    const raw = event.target.value;
                    void setSkillStrategy(
                      skill.name,
                      raw === "auto" ? null : (raw as LinkStrategyName)
                    );
                  }}
                  className="w-full cursor-pointer rounded-md border border-line bg-ink-950 px-2 py-1 text-[11.5px] text-text-muted outline-none"
                >
                  <option value="auto">{t("common.strategy.followPlatform")}</option>
                  {LINK_STRATEGIES.map((item) => {
                    const disabled = isStrategyDisabled(item.value, overview.strategies.platform);
                    return (
                      <option
                        key={item.value}
                        value={item.value}
                        disabled={disabled}
                        title={disabled ? junctionReason() : undefined}
                      >
                        {t(item.label)}
                        {disabled ? t("common.strategy.onlyWindows") : ""}
                      </option>
                    );
                  })}
                </select>
                <p className="mt-1 text-[11px] leading-relaxed text-text-dim">
                  {t("inspector.effective", {
                    name: strategyLabel(
                      overview.strategies.overrides[skill.name] ?? overview.strategies.default
                    ),
                  })}
                  {overview.strategies.overrides[skill.name]
                    ? t("common.strategy.overridden")
                    : t("common.strategy.followDefault")}
                </p>
                {!junctionSupported(overview.strategies.platform) && (
                  <p
                    data-junction-unsupported
                    className="mt-1 text-[11px] leading-relaxed text-text-dim"
                  >
                    {junctionReason()}
                  </p>
                )}
              </Section>
            )}

            <Section title={t("inspector.section.actions")}>
              <div className="flex flex-col gap-1.5">
                <Button size="sm" variant="outline" onClick={() => onOpenDiff(skill.name)}>
                  <GitCompare className="h-3 w-3" />
                  {t("inspector.compare")}
                </Button>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() => onInspectAgent(skill.agent ?? "")}
                >
                  <Boxes className="h-3 w-3" />
                  {t("inspector.locate")}
                </Button>
                {linkBadge(skill.link).tone === "danger" && (
                  <Button size="sm" variant="outline" onClick={() => void planRepair()}>
                    {t("inspector.planRepair")}
                  </Button>
                )}
              </div>
            </Section>
          </>
        ) : (
          <div className="px-3 py-4">
            <p className="text-[11.5px] leading-relaxed text-text-muted">
              {t("inspector.emptyHint")}
            </p>
            <div className="mt-3 flex flex-col gap-1.5">
              <Button size="sm" variant="outline" onClick={() => void planDedupe()}>
                {t("inspector.planDedupe")}
              </Button>
              <Button size="sm" variant="outline" onClick={() => void planRepair()}>
                {t("inspector.planRepair")}
              </Button>
              <Button size="sm" variant="ghost" onClick={() => void refresh()}>
                {t("inspector.rescan")}
              </Button>
              <Button size="sm" variant="ghost" onClick={() => onNavigate("changes")}>
                {t("inspector.viewHistory")}
              </Button>
            </div>
          </div>
        )}
      </div>
    </aside>
  );
}
