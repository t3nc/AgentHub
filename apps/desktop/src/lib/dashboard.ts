import type { MessageKey } from "./i18n/messages";
import type { DuplicateGroup, Overview } from "./types";

/**
 * 总览页的派生数据：指标、页签筛选、优先级。
 *
 * 全部是纯函数，不碰 IPC——扫描结果进来一次，页面里这些视图反复切换
 * 都不该再触发扫描。规模是「个位到几十个重复组」，一遍 filter 足够。
 */

export type DuplicateTab = "optimize" | "all" | "broken";

export const TAB_LABEL: Record<DuplicateTab, MessageKey> = {
  optimize: "dash.tab.optimize",
  all: "dash.tab.all",
  broken: "dash.tab.broken",
};

/** 组里只要有一个成员的链接断了，这组就不能「安全去重」，单独归到失效页签。 */
export function hasBrokenLink(group: DuplicateGroup): boolean {
  return group.members.some((member) => member.link.kind === "broken");
}

/** 可优化 = 确有可释放空间，且没有断链需要先修。 */
export function isOptimizable(group: DuplicateGroup): boolean {
  return group.wasted_bytes > 0 && !hasBrokenLink(group);
}

export function filterGroups(groups: DuplicateGroup[], tab: DuplicateTab): DuplicateGroup[] {
  if (tab === "broken") return groups.filter(hasBrokenLink);
  if (tab === "optimize") return groups.filter(isOptimizable);
  return groups;
}

export interface TabCounts {
  optimize: number;
  all: number;
  broken: number;
}

export function tabCounts(groups: DuplicateGroup[]): TabCounts {
  return {
    optimize: groups.filter(isOptimizable).length,
    all: groups.length,
    broken: groups.filter(hasBrokenLink).length,
  };
}

export type Priority = "high" | "medium" | "low";

export const PRIORITY_LABEL: Record<Priority, MessageKey> = {
  high: "dash.priority.high",
  medium: "dash.priority.medium",
  low: "dash.priority.low",
};

/** 阈值取整：≥1.5 MB 高、≥300 KB 建议，其余一般（数值来自真实扫描，不预设）。 */
const HIGH_BYTES = 1_500_000;
const MEDIUM_BYTES = 300_000;

export function priorityOf(wastedBytes: number): Priority {
  if (wastedBytes >= HIGH_BYTES) return "high";
  if (wastedBytes >= MEDIUM_BYTES) return "medium";
  return "low";
}

export interface DashboardMetrics {
  records: number;
  agents: number;
  groups: number;
  wastedBytes: number;
}

export function metricsOf(overview: Overview): DashboardMetrics {
  const { report } = overview;
  return {
    records: report.stats.total,
    agents: report.agents.filter((agent) => agent.detected).length,
    groups: report.duplicates.length,
    wastedBytes: report.stats.wasted_bytes,
  };
}

export interface IssueSummary {
  danger: number;
  warn: number;
  total: number;
}

/** 顶栏「待处理」胶囊与总览抬头的口径一致：只数 danger + warn，info 不算问题。 */
export function issuesOf(overview: Overview): IssueSummary {
  let danger = 0;
  let warn = 0;
  for (const issue of overview.doctor) {
    if (issue.level === "danger") danger += 1;
    else if (issue.level === "warn") warn += 1;
  }
  return { danger, warn, total: danger + warn };
}

/** 右栏「建议处理」：按可释放空间降序取前 N 条。 */
export function suggestionsOf(groups: DuplicateGroup[], limit = 2): DuplicateGroup[] {
  return [...groups]
    .filter((group) => group.wasted_bytes > 0)
    .sort((a, b) => b.wasted_bytes - a.wasted_bytes)
    .slice(0, limit);
}

/** 组里最适合作为「当前选择」的成员：优先 agent 侧副本，其次仓库真身。 */
export function representativeOf(group: DuplicateGroup) {
  return group.members.find((member) => member.agent) ?? group.members[0] ?? null;
}
