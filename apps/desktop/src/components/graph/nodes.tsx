import { memo } from "react";
import { clsx } from "clsx";
import { Handle, Position, type NodeProps, type Node } from "@xyflow/react";
import { AgentIcon } from "../agents/AgentIcon";
import { AgentHubMark } from "../brand/AgentHubMark";
import { agentInfo, bytes, shortPath } from "../../lib/format";
import { useT } from "../../lib/i18n";
import type { AgentGraphStat, Health } from "../../lib/graph";

export const STORE_W = 180;
export const STORE_H = 104;
export const AGENT_W = 196;
export const AGENT_H = 88;

const RING: Record<Health, string> = {
  ok: "border-state-ok/50",
  warn: "border-state-warn/55",
  danger: "border-state-danger/60",
};

const DOT: Record<Health, string> = {
  ok: "bg-state-ok",
  warn: "bg-state-warn",
  danger: "bg-state-danger",
};

/** 连线锚点用透明 Handle 承载：视觉上不画点，但让边贴在卡片边框上。 */
const hiddenHandle = { width: 1, height: 1, opacity: 0, border: "none" } as const;

export interface StoreNodeData extends Record<string, unknown> {
  path: string;
  skillCount: number;
  sharedSkills: number;
  sizeBytes: number;
  health: Health;
}

export interface AgentNodeData extends Record<string, unknown> {
  agent: AgentGraphStat;
}

function StoreNodeInner({ data, selected }: NodeProps<Node<StoreNodeData>>) {
  const t = useT();
  return (
    <div
      data-node="store"
      style={{ width: STORE_W, height: STORE_H }}
      className={clsx(
        "node-card flex flex-col justify-between px-3 py-2.5 text-left transition-colors",
        selected ? "border-brand-500 ring-1 ring-brand-500/30" : RING[data.health]
      )}
    >
      <Handle id="left" type="source" position={Position.Left} isConnectable={false} style={hiddenHandle} />
      <Handle id="right" type="source" position={Position.Right} isConnectable={false} style={hiddenHandle} />
      <Handle id="bottom" type="source" position={Position.Bottom} isConnectable={false} style={hiddenHandle} />

      <div className="flex items-center gap-2">
        <span className="flex h-7 w-7 items-center justify-center rounded-md bg-brand-500/12 text-brand-300">
          <AgentHubMark className="h-4 w-4" />
        </span>
        <div className="min-w-0">
          <p className="truncate text-[12.5px] font-medium text-text-strong">
            {t("common.store")}
          </p>
          <p className="truncate text-[11px] text-text-dim" title={data.path}>
            {shortPath(data.path, 2)}
          </p>
        </div>
      </div>
      <div className="flex items-center justify-between border-t border-line pt-1 text-[11.5px] text-text-muted">
        <span>{t("graph.skillCount", { count: data.skillCount })}</span>
        <span className="text-text-dim">{bytes(data.sizeBytes)}</span>
      </div>
      <p className="text-[11px] text-text-dim">
        {t("graph.sharedAcross", { count: data.sharedSkills })}
      </p>
    </div>
  );
}

function AgentNodeInner({ data, selected, targetPosition }: NodeProps<Node<AgentNodeData>>) {
  const t = useT();
  const { agent } = data;
  const outline =
    agent.broken > 0
      ? t("graph.broken", { count: agent.broken })
      : agent.copies > 0
        ? t("graph.copies", { count: agent.copies })
        : t("graph.linked", { count: agent.linked });

  return (
    <div
      data-node={`agent:${agent.id}`}
      style={{ width: AGENT_W, height: AGENT_H }}
      className={clsx(
        "node-card flex flex-col justify-between px-3 py-2.5 text-left transition-colors",
        selected ? "border-brand-500 ring-1 ring-brand-500/30" : RING[agent.health]
      )}
    >
      <Handle
        type="target"
        position={targetPosition ?? Position.Left}
        isConnectable={false}
        style={hiddenHandle}
      />
      <div className="flex items-start gap-2">
        <span
          className={clsx(
            "mt-0.5 flex h-6 w-6 items-center justify-center rounded border",
            agentInfo(agent.id).accent
          )}
        >
          <AgentIcon id={agent.id} className="h-4 w-4" />
        </span>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-1.5">
            <p className="truncate text-[12px] font-medium text-text-strong">{agent.name}</p>
            <span className={clsx("h-1.5 w-1.5 shrink-0 rounded-full", DOT[agent.health])} />
          </div>
          <p className="mono truncate text-[11px] text-text-dim" title={agent.path}>
            {shortPath(agent.path, 2)}
          </p>
        </div>
      </div>
      <div className="flex items-center justify-between text-[11.5px]">
        <span className="text-text-muted">{t("graph.skillCount", { count: agent.skillCount })}</span>
        <span
          className={clsx(
            agent.broken > 0
              ? "text-state-danger"
              : agent.copies > 0
                ? "text-state-warn"
                : "text-state-ok"
          )}
        >
          {outline}
        </span>
      </div>
    </div>
  );
}

export const StoreNode = memo(StoreNodeInner);
export const AgentNode = memo(AgentNodeInner);

/** 必须定义在模块级：每次 render 新建对象会让节点整棵重挂载。 */
export const nodeTypes = { store: StoreNode, agent: AgentNode };
