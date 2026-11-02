import { ChevronRight } from "lucide-react";
import { clsx } from "clsx";
import { Badge, Button } from "../ui";
import { AgentIcon } from "./AgentIcon";
import { InstallSteps } from "./InstallSteps";
import { KIND_LABEL, entryPath, managedAction, stateBadge } from "../../lib/agent-catalog";
import { agentInfo, shortPath } from "../../lib/format";
import { useT } from "../../lib/i18n";
import type { CatalogEntry } from "../../lib/types";

export interface AgentRowProps {
  entry: CatalogEntry;
  expanded: boolean;
  /** 已在「确认登记」态：行内展开路径清单 */
  pendingAdd: boolean;
  busy: boolean;
  onToggle: () => void;
  onRequestAdd: () => void;
  onCancelAdd: () => void;
  onConfirmAdd: () => void;
  onRemove: () => void;
  /** shared 场景：跳到接管它的内置 agent 的技能筛选 */
  onOpenShared: (agentId: string) => void;
  /** 行右键：交给 Agent 环境页弹菜单 */
  onContextMenu?: (event: React.MouseEvent) => void;
}

export function AgentRow({
  entry,
  expanded,
  pendingAdd,
  busy,
  onToggle,
  onRequestAdd,
  onCancelAdd,
  onConfirmAdd,
  onRemove,
  onOpenShared,
  onContextMenu,
}: AgentRowProps) {
  const t = useT();
  const state = stateBadge(entry);
  const action = managedAction(entry);
  const path = entryPath(entry);
  const open = expanded || pendingAdd;

  return (
    <div
      data-agent-row={entry.key}
      data-state={entry.state}
      data-ctx="agent-row"
      onContextMenu={onContextMenu}
      className={clsx(
        "border-t border-line first:border-t-0",
        entry.state === "missing" && "border-l-2 border-l-state-danger/60"
      )}
    >
      <div className="flex min-h-16 items-center gap-3 px-3 py-2">
        <span
          className="flex h-6 w-7 shrink-0 items-center justify-center rounded border border-line bg-surface-2 text-text-muted"
          title={t("agents.row.family", { family: entry.family })}
        >
          <AgentIcon id={entry.key} className="h-4 w-4" />
        </span>

        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-1.5">
            <span className="text-[12.5px] font-medium text-text-strong">{entry.name}</span>
            <Badge tone="dim">{t(KIND_LABEL[entry.kind])}</Badge>
            <Badge tone={state.tone}>{state.label}</Badge>
            {entry.builtin && <Badge tone="brand">{t("agents.badge.builtin")}</Badge>}
            {entry.managed === "managed" && (
              <Badge tone="brand">{t("agents.badge.managed")}</Badge>
            )}
            {entry.managed === "shared" && entry.shared_agent && (
              <Badge tone="brand">
                {t("agents.badge.shared", { name: agentInfo(entry.shared_agent).name })}
              </Badge>
            )}
            {entry.needs_confirm && (
              <Badge tone="warn">{t("agents.badge.needsConfirm")}</Badge>
            )}
          </div>
          <div className="mt-0.5 flex items-center gap-2">
            <span
              className="mono min-w-0 truncate text-[11.5px] text-text-dim"
              title={path || t("agents.row.noEntry")}
            >
              {path ? shortPath(path, 3) : t("agents.row.noEntry")}
            </span>
            {entry.version && (
              <span
                className="mono shrink-0 text-[11px] text-state-ok"
                title={entry.version}
              >
                {entry.version.slice(0, 28)}
              </span>
            )}
          </div>
        </div>

        <div className="hidden w-[168px] shrink-0 text-right xl:block">
          <span
            className="mono block truncate text-[11.5px] text-text-dim"
            title={entry.config_path ?? t("agents.row.noConfig")}
          >
            {entry.config_path ? shortPath(entry.config_path, 2) : "—"}
          </span>
          <span className="text-[11px] text-text-dim">
            {entry.config_exists ? t("agents.row.configExists") : t("agents.row.configMissing")}
          </span>
        </div>

        <button
          type="button"
          onClick={onToggle}
          aria-expanded={open}
          className="flex shrink-0 cursor-pointer items-center gap-1 rounded px-1.5 py-1 text-[11px] text-text-muted transition-colors hover:bg-surface-hover hover:text-text-strong"
        >
          {t("agents.row.installSteps")}
          <ChevronRight
            className={clsx("h-3 w-3 transition-transform", open && "rotate-90")}
          />
        </button>

        {action.kind === "jump" && action.agent && (
          <Button
            size="sm"
            variant="outline"
            onClick={() => onOpenShared(action.agent as string)}
            title={action.reason}
          >
            {action.label}
          </Button>
        )}
        {action.kind === "add" && (
          <Button size="sm" variant="outline" onClick={onRequestAdd} disabled={busy}>
            {action.label}
          </Button>
        )}
        {action.kind === "remove" && (
          <Button size="sm" variant="ghost" onClick={onRemove} disabled={busy}>
            {action.label}
          </Button>
        )}
        {action.kind === "none" && (
          <Button size="sm" variant="ghost" disabled title={action.reason}>
            {action.label}
          </Button>
        )}
      </div>

      {pendingAdd && (
        <div className="border-t border-line bg-surface-2/60 px-3 py-2.5">
          <p className="text-[12px] font-medium text-text-strong">
            {t("agents.row.registerTitle", { name: entry.name })}
          </p>
          {entry.managed_targets.length > 0 && (
            <ul className="mt-1.5 flex flex-col gap-1">
              {entry.managed_targets.map((target) => (
                <li key={`${target.scope}-${target.path}`} className="flex items-center gap-2">
                  <Badge tone={target.scope === "project" ? "dim" : "brand"}>
                    {target.label}
                  </Badge>
                  <code
                    className="mono min-w-0 flex-1 truncate text-[11.5px] text-text-muted"
                    title={target.path}
                  >
                    {target.path}
                  </code>
                  <span className="shrink-0 text-[11px] text-text-dim">
                    {target.exists ? t("agents.row.targetExists") : t("agents.row.targetMissing")}
                  </span>
                </li>
              ))}
            </ul>
          )}
          {entry.managed_note && (
            <p className="mt-1.5 text-[11.5px] leading-relaxed text-text-dim">
              {entry.managed_note}
            </p>
          )}
          <p className="mt-1 text-[11.5px] leading-relaxed text-brass">
            {t("agents.row.registerNote")}
          </p>
          <div className="mt-2 flex items-center gap-1.5">
            <Button size="sm" variant="primary" onClick={onConfirmAdd} disabled={busy}>
              {t("agents.row.confirmRegister")}
            </Button>
            <Button size="sm" variant="ghost" onClick={onCancelAdd} disabled={busy}>
              {t("common.cancel")}
            </Button>
          </div>
        </div>
      )}

      {expanded && <InstallSteps entry={entry} />}
    </div>
  );
}
