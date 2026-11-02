import { shortPath } from "./format";
import { tr } from "./i18n/runtime";
import type { MessageKey } from "./i18n/messages";
import type { DoctorIssue, Overview } from "./types";

/**
 * 顶栏「N 处待处理」的数据层。
 *
 * 后端 `doctor` 给出的是全量体检项（含 `info`），这里只挑真正要用户动手的
 * `danger` / `warn`，并按 `kind` 映射到具体的快捷操作——判断放在纯函数里，
 * 界面层不再写 `message.includes("...")` 这种会随文案漂移的逻辑。
 */

export type IssueAction = "repair" | "dedupe" | "locate" | "settings" | "none";

export interface PendingIssue {
  /** 只会是 danger / warn，info 不算「待处理」 */
  level: "danger" | "warn";
  path: string;
  /** 后端给的原文（已按当前语言返回），不做二次翻译 */
  message: string;
  /** 后端分类码；老壳没有时为空串 */
  kind: string;
  /** 技能名（收敛 / 定位需要） */
  subject: string | null;
  action: IssueAction;
  actionLabel: string;
  /** 展示用短路径 */
  short: string;
}

/** 分类码 → 动作 + 动作文案 key。 */
const ACTION_BY_KIND: Record<string, { action: IssueAction; label: MessageKey }> = {
  broken_link: { action: "repair", label: "issue.action.repair" },
  foreign_link: { action: "locate", label: "issue.action.view" },
  duplicate_copy: { action: "dedupe", label: "issue.action.dedupe" },
  store_missing: { action: "settings", label: "issue.action.settings" },
};

const CATEGORY: Record<string, MessageKey> = {
  broken_link: "issue.cat.broken",
  foreign_link: "issue.cat.foreign",
  duplicate_copy: "issue.cat.duplicate",
  store_missing: "issue.cat.storeMissing",
};

const LEVEL_RANK: Record<PendingIssue["level"], number> = { danger: 0, warn: 1 };

function actionOf(issue: DoctorIssue): { action: IssueAction; label: string } {
  const known = ACTION_BY_KIND[issue.kind ?? ""];
  if (known) return { action: known.action, label: tr(known.label) };
  // 分类码缺失（老壳）或本版本还不认识的类型：只给「查看」，绝不猜写操作
  return { action: "locate", label: tr("issue.action.view") };
}

/** 体检项 → 待处理清单：只留 danger/warn，危险项排前面。 */
export function pendingIssues(overview: Overview | null): PendingIssue[] {
  if (!overview) return [];
  const out: PendingIssue[] = [];
  for (const issue of overview.doctor) {
    if (issue.level !== "danger" && issue.level !== "warn") continue;
    const meta = actionOf(issue);
    out.push({
      level: issue.level === "danger" ? "danger" : "warn",
      path: issue.path,
      message: issue.message,
      kind: issue.kind ?? "",
      subject: issue.subject ?? null,
      action: meta.action,
      actionLabel: meta.label,
      short: shortPath(issue.path, 3),
    });
  }
  out.sort((a, b) => LEVEL_RANK[a.level] - LEVEL_RANK[b.level]);
  return out;
}

/** 「1 处失效链接、2 处待收敛副本」——顶栏悬停与浮层标题共用。 */
export function issuesSummary(issues: PendingIssue[]): string {
  if (issues.length === 0) return tr("issue.none");
  const counts = new Map<string, number>();
  for (const issue of issues) {
    const label = tr(CATEGORY[issue.kind] ?? "issue.summary.unknown");
    counts.set(label, (counts.get(label) ?? 0) + 1);
  }
  return Array.from(counts, ([label, count]) =>
    tr("issue.summaryItem", { count, label })
  ).join(tr("issue.summarySep"));
}

/** 顶栏胶囊文案：`2 处待处理`。 */
export function issuesLabel(issues: PendingIssue[]): string {
  return issues.length > 0 ? tr("issue.count", { count: issues.length }) : tr("issue.none");
}

/** 是否有可执行的批量修复（只有存在失效链接时才有意义）。 */
export function canRepairAll(issues: PendingIssue[]): boolean {
  return issues.some((issue) => issue.action === "repair");
}
