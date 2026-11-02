import { toSlash } from "./format";
import { tr } from "./i18n/runtime";

/**
 * 终端输出里的「可点东西」：路径和批次号。
 *
 * 全部是纯函数，不发 IPC——归属判断用的就是页面已经有的数据
 * （store_root、各 agent 的 skills 目录、项目根）。
 *
 * 安全取向：**只给受管范围内的路径生成链接**。终端里冒出来的
 * `C:\Windows\System32` 之类不会被点成「跳转」，免得把任意路径变成入口。
 */

export type LinkKind = "skill" | "project" | "mcp" | "batch";

export interface LinkTarget {
  kind: LinkKind;
  /** skill 用技能名，batch 用批次号，project 用项目根 */
  id: string;
  label: string;
}

export interface LinkContext {
  /**
   * 扫描结果里的每条技能目录（agent 下的副本与中央仓库的真身都算）。
   * 直接用 records 而不是 settings：`overview` 一定先于终端可用，
   * 而 settings 是懒加载的，拿来判断归属会时灵时不灵。
   */
  skills: { name: string; path: string }[];
  projects: { root: string; name: string }[];
  /** 用户主目录，用来展开 `~/` */
  home: string | null;
}

export interface LinkHit {
  /** 行内起始列（0-based） */
  start: number;
  length: number;
  target: LinkTarget;
}

/**
 * `cs-20261030-212559-4260-956-000`：日期-时间-进程号-毫秒-序号，
 * 后三段来自 `paths::unique_id()` 的 4/3/3 位。
 */
const BATCH_SOURCE = String.raw`cs-\d{8}-\d{6}-\d{4}-\d{3}-\d{3}`;
const BATCH_FULL = new RegExp(`^${BATCH_SOURCE}$`);
/** 绝对路径：盘符、UNC、Unix、`~/` */
const PATH_PATTERN = /(?:[A-Za-z]:[\\/]|\\\\|~[\\/]|\/)[^\s"'<>|?*]*/g;

const MCP_SUFFIX = /(?:^|\/)(?:\.mcp\.json|mcp\.json|\.codex\/config\.toml)$/;

function normalize(value: string): string {
  // Windows 路径大小写不敏感；统一成 `/` + 小写后再比前缀
  return toSlash(value.trim()).replace(/\/+$/, "").toLowerCase();
}

function expandHome(raw: string, home: string | null): string {
  if (!home) return raw;
  if (raw === "~") return home;
  if (raw.startsWith("~/") || raw.startsWith("~\\")) return `${home}/${raw.slice(2)}`;
  return raw;
}

/** 单个片段 → 跳转目标；不在受管范围内返回 null。 */
export function resolveTarget(rawInput: string, ctx: LinkContext): LinkTarget | null {
  const raw = rawInput.trim();
  if (!raw) return null;

  if (BATCH_FULL.test(raw)) {
    return { kind: "batch", id: raw, label: tr("link.batch", { id: raw }) };
  }

  const full = normalize(expandHome(raw, ctx.home));
  if (full.length < 3) return null;

  // 1) 技能目录：最长前缀优先（同一技能在多个 agent 下各有一份路径）
  const skills = ctx.skills
    .map((skill) => ({ ...skill, dir: normalize(skill.path) }))
    .filter((skill) => skill.dir.length > 2)
    .sort((a, b) => b.dir.length - a.dir.length);
  for (const skill of skills) {
    if (full === skill.dir || full.startsWith(`${skill.dir}/`)) {
      return { kind: "skill", id: skill.name, label: tr("link.skill", { name: skill.name }) };
    }
  }

  // 2) 项目根
  for (const project of ctx.projects) {
    const root = normalize(project.root);
    if (root && (full === root || full.startsWith(`${root}/`))) {
      return {
        kind: "project",
        id: project.root,
        label: tr("link.project", { name: project.name }),
      };
    }
  }

  // 3) MCP 配置文件
  if (MCP_SUFFIX.test(full)) {
    return { kind: "mcp", id: raw, label: tr("link.mcp") };
  }

  return null;
}

/**
 * 扫一行文本，返回所有可点片段。
 *
 * 前置判断很便宜：不含分隔符与 `cs-` 的行直接返回——终端滚动时
 * 这个函数按可见行调用，不能每行都跑两个正则。
 */
export function scanLinks(line: string, ctx: LinkContext): LinkHit[] {
  if (!line || line.length > 512) return [];
  if (!/[\\/~]/.test(line) && !line.includes("cs-")) return [];

  const hits: LinkHit[] = [];
  const taken: Array<[number, number]> = [];

  const overlaps = (start: number, end: number) =>
    taken.some(([from, to]) => start < to && end > from);

  for (const match of line.matchAll(new RegExp(BATCH_SOURCE, "g"))) {
    if (match.index === undefined) continue;
    const start = match.index;
    const end = start + match[0].length;
    hits.push({
      start,
      length: match[0].length,
      target: {
        kind: "batch",
        id: match[0],
        label: tr("link.batch", { id: match[0] }),
      },
    });
    taken.push([start, end]);
  }

  for (const match of line.matchAll(PATH_PATTERN)) {
    if (match.index === undefined) continue;
    const start = match.index;
    const end = start + match[0].length;
    if (overlaps(start, end)) continue;

    // URL 的 `//host`、标识符里的 `/` 不是路径
    const before = start > 0 ? line[start - 1] : "";
    if (before && /[A-Za-z0-9:_]/.test(before)) continue;

    const target = resolveTarget(match[0], ctx);
    if (!target) continue;
    hits.push({ start, length: match[0].length, target });
    taken.push([start, end]);
  }

  return hits.sort((a, b) => a.start - b.start);
}

/** 供测试与调试：把一行拆成「片段 + 目标」的可读结果 */
export interface LinkDebug {
  buffer: { start: number; length: number };
  target: LinkTarget;
}

export function debugScan(line: string, ctx: LinkContext): LinkDebug[] {
  return scanLinks(line, ctx).map((hit) => ({
    buffer: { start: hit.start, length: hit.length },
    target: hit.target,
  }));
}
