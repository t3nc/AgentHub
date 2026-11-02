import { tr } from "./i18n/runtime";
import type { AgentInstall, DuplicateGroup, Overview, SkillRecord } from "./types";

export type Health = "ok" | "warn" | "danger";

export interface AgentGraphStat {
  id: string;
  name: string;
  path: string;
  cliPath: string | null;
  detected: boolean;
  capabilities: AgentInstall["capabilities"];
  skillCount: number;
  linked: number;
  copies: number;
  broken: number;
  foreign: number;
  readonly: number;
  sizeBytes: number;
  health: Health;
}

export interface GraphNodeModel {
  id: string;
  kind: "store" | "agent";
  label: string;
  path: string;
  skillCount: number;
  health: Health;
}

export interface GraphEdgeModel {
  id: string;
  source: string;
  target: string;
  health: Health;
  /** 连线上的短标签，例如「4 已接管」「2 份副本」 */
  label: string;
}

export interface StoreStat {
  path: string;
  skillCount: number;
  sizeBytes: number;
  sharedSkills: number;
  health: Health;
}

export interface GraphModel {
  nodes: GraphNodeModel[];
  edges: GraphEdgeModel[];
  agents: AgentGraphStat[];
  store: StoreStat;
  totals: { linked: number; copies: number; broken: number; foreign: number };
}

export const STORE_NODE_ID = "store";
export const agentNodeId = (id: string) => `agent:${id}`;

function healthOf(broken: number, copies: number, foreign: number): Health {
  if (broken > 0) return "danger";
  if (copies > 0 || foreign > 0) return "warn";
  return "ok";
}

function edgeLabel(linked: number, copies: number, broken: number): string {
  if (broken > 0) return tr("graph.broken", { count: broken });
  if (copies > 0) return tr("graph.copies", { count: copies });
  return tr("graph.linked", { count: linked });
}

/**
 * 把一次扫描结果压成「仓库 → 各 agent」的图谱模型。
 *
 * 纯函数：节点数固定为 1 + agent 数，不随技能总数增长；技能明细通过
 * 选中对象在检查器里按需查询，避免把上千条记录塞进画布。
 */
export function buildGraph(overview: Overview): GraphModel {
  const { report } = overview;
  const agents: AgentGraphStat[] = report.agents.map((agent) => {
    const records = report.records.filter((r) => r.agent === agent.id);
    const linked = records.filter((r) =>
      ["symlink", "junction", "hardlink"].includes(r.link.kind)
    ).length;
    const copies = records.filter((r) => r.link.kind === "real_dir" && !r.readonly).length;
    const broken = records.filter((r) => r.link.kind === "broken").length;
    const foreign = records.filter((r) => r.link.kind === "foreign").length;
    const readonly = records.filter((r) => r.readonly).length;
    return {
      id: agent.id,
      name: agent.display_name,
      path: agent.config_dir ?? "—",
      cliPath: agent.cli_path,
      detected: agent.detected,
      capabilities: agent.capabilities,
      skillCount: records.length,
      linked,
      copies,
      broken,
      foreign,
      readonly,
      sizeBytes: records.reduce((sum, r) => sum + r.size_bytes, 0),
      health: healthOf(broken, copies, foreign),
    };
  });

  const totals = {
    linked: agents.reduce((n, a) => n + a.linked, 0),
    copies: agents.reduce((n, a) => n + a.copies, 0),
    broken: agents.reduce((n, a) => n + a.broken, 0),
    foreign: agents.reduce((n, a) => n + a.foreign, 0),
  };

  const store: StoreStat = {
    path: report.store_root,
    skillCount: report.stats.store_skills,
    sizeBytes: report.stats.store_bytes,
    sharedSkills: report.stats.shared_skills,
    health: healthOf(totals.broken, totals.copies, totals.foreign),
  };

  const nodes: GraphNodeModel[] = [
    {
      id: STORE_NODE_ID,
      kind: "store",
      label: tr("common.store"),
      path: store.path,
      skillCount: store.skillCount,
      health: store.health,
    },
    ...agents.map((agent) => ({
      id: agentNodeId(agent.id),
      kind: "agent" as const,
      label: agent.name,
      path: agent.path,
      skillCount: agent.skillCount,
      health: agent.health,
    })),
  ];

  const edges: GraphEdgeModel[] = agents.map((agent) => ({
    id: `${STORE_NODE_ID}->${agent.id}`,
    source: STORE_NODE_ID,
    target: agentNodeId(agent.id),
    health: agent.health,
    label: edgeLabel(agent.linked, agent.copies, agent.broken),
  }));

  return { nodes, edges, agents, store, totals };
}

/** 选中对象 → 详情，供检查器使用。 */
export function findAgent(model: GraphModel, id: string): AgentGraphStat | null {
  return model.agents.find((agent) => agent.id === id) ?? null;
}

export function findSkill(overview: Overview, path: string): SkillRecord | null {
  return overview.report.records.find((record) => record.path === path) ?? null;
}

export function skillsOfAgent(overview: Overview, agentId: string): SkillRecord[] {
  return overview.report.records.filter((record) => record.agent === agentId);
}

export function duplicateOf(
  overview: Overview,
  name: string
): DuplicateGroup | null {
  return overview.report.duplicates.find((group) => group.name === name) ?? null;
}

/** 重复组里「可省但还没接管」的字节数，用于底部面板的预计节省。 */
export function savingsOf(groups: DuplicateGroup[]): number {
  return groups.reduce((sum, group) => sum + group.wasted_bytes, 0);
}
