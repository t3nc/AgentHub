import { tr } from "./i18n/runtime";
import type { WatchDelta } from "./types";

/**
 * 外部变更摘要的纯函数层。
 *
 * 壳层推来的 `WatchDelta` 是「相对上一次快照」的增量，同一个文件在短时间内可能
 * 出现在多条事件里；提示条要的是「现在有哪些东西变了」，所以这里做合并与去重。
 * 摘要则负责把路径翻译成用户能懂的结论（技能变更 / 配置变更）。
 */

export const EMPTY_DELTA: WatchDelta = { added: [], removed: [], changed: [] };

function uniqSorted(paths: string[]): string[] {
  return Array.from(new Set(paths)).sort((a, b) => a.localeCompare(b, "zh-CN"));
}

/** 合并两份变更摘要：同一路径只保留一次，三个桶各自去重排序。 */
export function mergeDelta(prev: WatchDelta | null, next: WatchDelta): WatchDelta {
  if (!prev) {
    return {
      added: uniqSorted(next.added),
      removed: uniqSorted(next.removed),
      changed: uniqSorted(next.changed),
    };
  }
  // 一个文件先新增后修改时，added 比 changed 更能说明发生过什么：保留 added。
  const added = uniqSorted([...prev.added, ...next.added]);
  const addedSet = new Set(added);
  const changed = uniqSorted([...prev.changed, ...next.changed]).filter(
    (path) => !addedSet.has(path)
  );
  // 已经删除的路径不该同时算「变更」；重新出现则按新增处理（上一步已覆盖）。
  const removed = uniqSorted([...prev.removed, ...next.removed]).filter(
    (path) => !addedSet.has(path)
  );
  return { added, removed, changed };
}

export function deltaTotal(delta: WatchDelta | null): number {
  if (!delta) return 0;
  return delta.added.length + delta.removed.length + delta.changed.length;
}

/** agent 配置文件（Provider / MCP / 指令）——命中这些说明是配置改动而不是技能改动。 */
const CONFIG_NAMES = new Set([
  ".claude.json",
  ".mcp.json",
  "agents.md",
  "claude.md",
  "config.json",
  "config.toml",
  "config.yaml",
  "hooks.json",
  "mcp.json",
  "settings.json",
  "settings.local.json",
  "soul.md",
]);

export function baseName(path: string): string {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

/** 展示用：只留末尾两段，避免顶栏被长路径撑开。 */
export function shortOf(path: string, depth = 2): string {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts.slice(-depth).join("/");
}

export interface FsChangeSummary {
  /** skill = 只有技能目录变了；config = 只有配置文件变了；mixed = 两者都有 */
  kind: "skill" | "config" | "mixed";
  /** 提示条上的短结论，例如「技能变更 · 2 项」 */
  label: string;
  /** 悬停展示的完整清单（短路径） */
  detail: string[];
  total: number;
}

export function summarizeDelta(delta: WatchDelta | null): FsChangeSummary {
  const all = delta ? [...delta.added, ...delta.removed, ...delta.changed] : [];
  const total = all.length;
  const configs = all.filter((path) => CONFIG_NAMES.has(baseName(path).toLowerCase()));
  const skills = total - configs.length;

  const detail = all.slice(0, 10).map((path) => shortOf(path));
  if (total > 10) detail.push(tr("fs.more", { count: total }));

  if (total === 0) {
    return { kind: "skill", label: "", detail: [], total: 0 };
  }
  if (configs.length === 0) {
    return { kind: "skill", label: tr("fs.skill", { count: total }), detail, total };
  }
  if (skills === 0) {
    const label =
      configs.length === 1
        ? baseName(configs[0])
        : tr("fs.configsMany", { count: configs.length });
    return { kind: "config", label: tr("fs.configWith", { name: label }), detail, total };
  }
  return {
    kind: "mixed",
    label: tr("fs.both", { skills, configs: configs.length }),
    detail,
    total,
  };
}

/** 悬停提示：一行一条，最多 10 条（`detail` 已截断）。 */
export function deltaTooltip(delta: WatchDelta | null): string {
  const summary = summarizeDelta(delta);
  if (summary.total === 0) return "";
  const head = tr("fs.delta", {
    added: delta?.added.length ?? 0,
    removed: delta?.removed.length ?? 0,
    changed: delta?.changed.length ?? 0,
  });
  return [head, ...summary.detail].join("\n");
}
