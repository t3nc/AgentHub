import { agentInfo, bytes, shortPath, tailPath, timeAgo } from "./format";
import { currentLocale, tr } from "./i18n/runtime";
import { bcp47 } from "./i18n/types";
import type { MessageKey } from "./i18n/messages";
import type { LinkState, SkillRecord } from "./types";

export type Verdict = "same" | "diff" | "broken" | "single";

export interface DiffMember {
  record: SkillRecord;
  /** 列头展示用：agent 名或「中央仓库」 */
  label: string;
  /** 来源标签：中央仓库 / agent 副本 / 只读内置 */
  sourceLabel: string;
  /** 能否作为基线（只有实体目录副本可以） */
  usable: boolean;
  reason?: string;
}

export interface DiffRow {
  key: string;
  label: string;
  values: string[];
  /** 所有成员该属性一致；一致行在界面里弱化 */
  same: boolean;
  /** 只有一处不同时，指向那一列的索引（用于强调） */
  outlier: number | null;
}

export interface SkillDiff {
  name: string;
  members: DiffMember[];
  verdict: Verdict;
  summary: string;
  identical: boolean;
  hashCount: number;
  brokenCount: number;
  readonlyCount: number;
  foreignCount: number;
  /** 默认基线（可用副本里体积最大的那份） */
  candidate: string | null;
  rows: DiffRow[];
}

/** 来源标签用 key 而不是文本：同一个枚举在列头、行值、基线提示里各出现一次。 */
const SOURCE_KEY: Record<SkillRecord["source"], MessageKey> = {
  store: "diff.source.store",
  agent_copy: "diff.source.agentCopy",
  readonly: "diff.source.readonly",
};

function linkLabel(link: LinkState): string {
  switch (link.kind) {
    case "symlink":
    case "junction":
    case "hardlink":
      return tr("diff.link.linkTo", { path: tailPath(link.target, 2) });
    case "broken":
      return tr("diff.link.broken");
    case "foreign":
      return tr("diff.link.foreign");
    default:
      return tr("diff.link.realDir");
  }
}

function linkKind(link: LinkState): string {
  return link.kind;
}

/** 基线只能是实体目录副本：链接搬进仓库会变成坏链，仓库里的那份已经在仓库了。 */
function baselineReason(record: SkillRecord): string | undefined {
  if (record.readonly) return tr("diff.reason.readonly");
  if (record.source === "store") return tr("diff.reason.inStore");
  if (record.link.kind === "foreign") return tr("diff.reason.foreign");
  if (record.link.kind !== "real_dir") return tr("diff.reason.notRealDir");
  return undefined;
}

function buildRow(
  key: string,
  label: string,
  raw: (string | number)[],
  display: string[],
  forcedDiff = false
): DiffRow {
  const first = raw[0];
  const same = !forcedDiff && raw.every((value) => value === first);
  let outlier: number | null = null;
  if (!same && raw.length > 1) {
    const counts = new Map<string | number, number>();
    raw.forEach((value) => counts.set(value, (counts.get(value) ?? 0) + 1));
    // 只容忍「多数一致 + 单个异类」的情况，多个版本时不做单点强调
    const majority = [...counts.entries()].sort((a, b) => b[1] - a[1])[0];
    if (majority && majority[1] === raw.length - 1) {
      outlier = raw.findIndex((value) => value !== majority[0]);
    }
  }
  return { key, label, values: display, same, outlier };
}

/**
 * 把同名技能的多份记录整理成「列 = 一份、行 = 一个属性」的对比模型。
 * 直接吃 `report.records`：破损链接与只有一份的技能都要能看，
 * 不能只依赖 `duplicates`（那里排除了 readonly 与坏链）。
 */
export function buildSkillDiff(records: SkillRecord[], name: string): SkillDiff | null {
  const mine = records.filter((record) => record.name === name);
  if (mine.length === 0) return null;

  // 列顺序按当前界面的排序规则：中文按拼音、英文按字母，用户看列表时的顺序才不跳
  const collator = bcp47(currentLocale());
  const ordered = [...mine].sort((a, b) => {
    if (a.source === "store" && b.source !== "store") return -1;
    if (b.source === "store" && a.source !== "store") return 1;
    return agentInfo(a.agent).name.localeCompare(agentInfo(b.agent).name, collator);
  });

  const members: DiffMember[] = ordered.map((record) => {
    const reason = baselineReason(record);
    return {
      record,
      label: record.source === "store" ? tr("diff.source.store") : agentInfo(record.agent).name,
      sourceLabel: tr(SOURCE_KEY[record.source]),
      usable: reason === undefined,
      reason,
    };
  });

  const hashes = new Set(mine.map((r) => r.tree_hash ?? "").filter((h) => h.length > 0));
  const hashCount = hashes.size;
  const brokenCount = mine.filter((r) => r.link.kind === "broken").length;
  const readonlyCount = mine.filter((r) => r.readonly).length;
  const foreignCount = mine.filter((r) => r.link.kind === "foreign").length;
  const identical = hashCount <= 1;

  const verdict: Verdict =
    hashCount > 1 ? "diff" : brokenCount > 0 ? "broken" : mine.length === 1 ? "single" : "same";

  const summary =
    verdict === "diff"
      ? tr("diff.summary.diff", { versions: hashCount, copies: mine.length })
      : verdict === "broken"
        ? tr("diff.summary.broken", { count: brokenCount })
        : verdict === "single"
          ? tr("diff.summary.single")
          : tr("diff.summary.same", { count: mine.length });

  const candidate =
    mine
      .filter((record) => baselineReason(record) === undefined)
      .sort((a, b) => b.size_bytes - a.size_bytes)[0]?.path ?? null;

  const rows: DiffRow[] = [
    buildRow(
      "path",
      tr("diff.row.path"),
      mine.map((r) => r.path),
      mine.map((r) => shortPath(r.path, 3)),
      // 路径必然不同，不靠它判断「版本是否一致」
      true
    ),
    buildRow(
      "source",
      tr("diff.row.source"),
      mine.map((r) => r.source),
      mine.map((r) => tr(SOURCE_KEY[r.source]))
    ),
    buildRow(
      "link",
      tr("diff.row.link"),
      mine.map((r) => linkKind(r.link)),
      mine.map((r) => linkLabel(r.link))
    ),
    buildRow(
      "hash",
      tr("diff.row.hash"),
      mine.map((r) => r.tree_hash ?? ""),
      mine.map((r) => (r.tree_hash ? r.tree_hash.slice(0, 8) : "—"))
    ),
    buildRow(
      "size",
      tr("diff.row.size"),
      mine.map((r) => r.size_bytes),
      mine.map((r) => bytes(r.size_bytes))
    ),
    buildRow(
      "files",
      tr("diff.row.files"),
      mine.map((r) => r.file_count),
      mine.map((r) => String(r.file_count))
    ),
    buildRow(
      "mtime",
      tr("diff.row.mtime"),
      mine.map((r) => r.modified_at),
      mine.map((r) => timeAgo(r.modified_at)),
      true
    ),
    buildRow(
      "description",
      tr("diff.row.description"),
      mine.map((r) => (r.description ?? "").trim()),
      mine.map((r) => (r.description ?? "—").trim())
    ),
  ];

  return {
    name,
    members,
    verdict,
    summary,
    identical,
    hashCount,
    brokenCount,
    readonlyCount,
    foreignCount,
    candidate,
    rows,
  };
}

/** 结论徽章的配色语义，供组件直接映射到 Tailwind 类。 */
export const VERDICT_LABEL: Record<Verdict, MessageKey> = {
  same: "diff.verdict.same",
  diff: "diff.verdict.diff",
  broken: "diff.verdict.broken",
  single: "diff.verdict.single",
};
