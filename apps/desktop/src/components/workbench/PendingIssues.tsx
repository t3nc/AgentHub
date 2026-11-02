import { useEffect, useRef, useState } from "react";
import { AlertTriangle, ChevronDown, RefreshCw, Wrench, X } from "lucide-react";
import { clsx } from "clsx";
import { Badge, Button } from "../ui";
import { useHub } from "../../lib/useHub";
import { useT } from "../../lib/i18n";
import { useSelection } from "../../lib/selection";
import {
  issuesLabel,
  issuesSummary,
  pendingIssues,
  type PendingIssue,
} from "../../lib/issues";

/**
 * 「N 处待处理」的清单与浮层。
 *
 * 顶栏只给一个数字，用户看不出要处理什么、也没法直接动手——所以这里把
 * `overview.doctor` 的明细摊开，并按 `kind` 给一条对应动作（修复 / 收敛 / 查看 / 去设置）。
 */

export interface PendingIssueHandlers {
  /** 跳到设置页（中央仓库路径等装配期问题） */
  onOpenSettings?: () => void;
  /** 「查看全部」：滚到总览页的待处理卡片 */
  onShowAll?: () => void;
  onLocate?: (issue: PendingIssue) => void;
}

/** 把一条问题落到动作上。所有写操作都走计划抽屉，不会就地改盘。 */
function useIssueRunner({ onOpenSettings, onLocate }: PendingIssueHandlers) {
  const { planDedupe, planRepair, notify } = useHub();
  const { select } = useSelection();
  const t = useT();

  return (issue: PendingIssue) => {
    switch (issue.action) {
      case "repair":
        void planRepair();
        return;
      case "dedupe":
        void planDedupe(issue.subject ? [issue.subject] : undefined);
        return;
      case "settings":
        if (onOpenSettings) onOpenSettings();
        else notify(t("issue.settingsMissing"));
        return;
      case "locate":
        select({ kind: "skill", id: issue.path });
        onLocate?.(issue);
        return;
      default:
        return;
    }
  };
}

function IssueRow({
  issue,
  onRun,
}: {
  issue: PendingIssue;
  onRun: (issue: PendingIssue) => void;
}) {
  return (
    <div className="flex items-start gap-2 rounded-md px-1.5 py-1.5 transition-colors hover:bg-surface-hover">
      <span
        aria-hidden
        className={clsx(
          "mt-[2px] w-[2px] self-stretch rounded-full",
          issue.level === "danger" ? "bg-state-danger" : "bg-state-warn"
        )}
      />
      <div className="min-w-0 flex-1">
        <p className="truncate text-[12px] leading-snug text-text-strong" title={issue.message}>
          {issue.message}
        </p>
        <code
          className="mono mt-0.5 block truncate text-[11.5px] text-text-dim"
          title={issue.path}
        >
          {issue.short}
        </code>
      </div>
      {issue.action !== "none" && (
        <button
          type="button"
          onClick={() => onRun(issue)}
          className="mt-[1px] shrink-0 cursor-pointer rounded-md border border-line px-1.5 py-[3px] text-[11.5px] text-text-muted transition-colors hover:border-line-strong hover:bg-surface-2 hover:text-text-strong"
        >
          {issue.actionLabel}
        </button>
      )}
    </div>
  );
}

/** 顶栏触发按钮 + 右对齐浮层清单。 */
export function PendingIssuesButton({
  onOpenSettings,
  onShowAll,
  onLocate,
}: PendingIssueHandlers) {
  const { overview, rescan, loading } = useHub();
  const t = useT();
  const issues = pendingIssues(overview);
  const run = useIssueRunner({ onOpenSettings, onLocate });
  const [open, setOpen] = useState(false);
  const boxRef = useRef<HTMLDivElement | null>(null);

  // 点外面 / Esc 收起：顶栏是常驻的，浮层不能只靠再点一次按钮才能关
  useEffect(() => {
    if (!open) return;
    const onDown = (event: MouseEvent) => {
      if (!boxRef.current?.contains(event.target as Node)) setOpen(false);
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  const danger = issues.filter((issue) => issue.level === "danger").length;
  const tone = danger > 0 ? "danger" : issues.length > 0 ? "warn" : "ok";

  // 没有待处理项时不显示可点击态：一个点了没反应的按钮比一个数字更让人困惑
  if (issues.length === 0) {
    return (
      <span
        title={
          overview?.report
            ? t("issue.healthOkWithScan", { ms: overview.report.duration_ms })
            : t("issue.healthOk")
        }
        className="hidden items-center gap-1.5 rounded-md border border-line bg-surface-2 px-2 py-1 text-[11.5px] text-state-ok sm:flex"
      >
        <span className="h-1.5 w-1.5 rounded-full bg-state-ok" />
        {t("issue.none")}
      </span>
    );
  }

  return (
    <div ref={boxRef} className="relative hidden sm:block">
      <button
        type="button"
        onClick={() => setOpen((prev) => !prev)}
        aria-haspopup="dialog"
        aria-expanded={open}
        title={t("issue.openHint", { summary: issuesSummary(issues) })}
        className={clsx(
          "flex cursor-pointer items-center gap-1.5 rounded-md border px-2 py-1 text-[11.5px] transition-colors",
          tone === "danger"
            ? "border-state-danger/40 bg-state-danger/10 text-state-danger hover:border-state-danger/70"
            : "border-state-warn/40 bg-state-warn/10 text-state-warn hover:border-state-warn/70"
        )}
      >
        <AlertTriangle className="h-3 w-3" />
        {issuesLabel(issues)}
        <ChevronDown
          className={clsx(
            "h-3 w-3 opacity-60 transition-transform duration-150",
            open && "rotate-180"
          )}
        />
      </button>

      {open && (
        <div
          role="dialog"
          aria-label={t("issue.dialog")}
          className="absolute right-0 top-full z-50 mt-2 w-[380px] overflow-hidden rounded-xl border border-line bg-ink-900 shadow-panel animate-in fade-in slide-in-from-top-1 duration-150 motion-reduce:animate-none"
        >
          <header className="flex items-center justify-between gap-2 border-b border-line px-3 py-2">
            <div className="flex min-w-0 items-center gap-2">
              <h3 className="text-[12.5px] font-semibold text-text-strong">
                {t("issue.header")}
              </h3>
              <Badge tone={tone === "danger" ? "danger" : "warn"}>
                {t("common.places", { count: issues.length })}
              </Badge>
            </div>
            <button
              type="button"
              onClick={() => setOpen(false)}
              aria-label={t("issue.close")}
              className="shrink-0 cursor-pointer rounded p-0.5 text-text-dim transition-colors hover:text-text-strong"
            >
              <X className="h-3.5 w-3.5" />
            </button>
          </header>

          <div className="max-h-[320px] overflow-y-auto px-1.5 py-1.5">
            {issues.slice(0, 6).map((issue) => (
              <IssueRow
                key={`${issue.kind}:${issue.path}`}
                issue={issue}
                // 动作都会打开计划抽屉或跳页，浮层留在这里只会挡住结果
                onRun={(next) => {
                  run(next);
                  setOpen(false);
                }}
              />
            ))}
            {issues.length > 6 && (
              <p className="px-1.5 py-1 text-[11.5px] text-text-dim">
                {t("issue.moreCompact", { count: issues.length - 6 })}
              </p>
            )}
          </div>

          <footer className="flex items-center justify-between gap-2 border-t border-line px-3 py-2">
            <button
              type="button"
              onClick={() => {
                setOpen(false);
                onShowAll?.();
              }}
              className="cursor-pointer text-[11px] text-text-muted transition-colors hover:text-text-strong"
            >
              {t("issue.viewAll")} →
            </button>
            <Button size="sm" variant="ghost" disabled={loading} onClick={() => void rescan()}>
              <RefreshCw className={clsx("h-3 w-3", loading && "animate-spin")} />
              {t("shell.rescan")}
            </Button>
          </footer>
        </div>
      )}
    </div>
  );
}

/** 总览页内嵌的问题卡片：与浮层同一份行组件，只是不折叠。 */
export function PendingIssuesList({
  onOpenSettings,
  onLocate,
}: PendingIssueHandlers) {
  const { overview, planRepair, loading } = useHub();
  const t = useT();
  const issues = pendingIssues(overview);
  const run = useIssueRunner({ onOpenSettings, onLocate });

  if (issues.length === 0) return null;

  const danger = issues.filter((issue) => issue.level === "danger").length;
  const repair = issues.some((issue) => issue.action === "repair");

  return (
    <section
      id="pending-issues"
      className="panel overflow-hidden transition-shadow duration-300"
    >
      <header className="flex flex-wrap items-center justify-between gap-2 border-b border-line px-4 py-2.5">
        <div className="flex min-w-0 items-center gap-2">
          <Wrench
            className={clsx(
              "h-3.5 w-3.5",
              danger > 0 ? "text-state-danger" : "text-state-warn"
            )}
          />
          <h2 className="text-[13.5px] font-semibold text-text-strong">{t("issue.header")}</h2>
          <Badge tone={danger > 0 ? "danger" : "warn"}>
            {t("common.places", { count: issues.length })}
          </Badge>
          <p className="hidden truncate text-[11.5px] text-text-dim sm:block">
            {issuesSummary(issues)}
          </p>
        </div>
        {repair && (
          <Button size="sm" variant="outline" disabled={loading} onClick={() => void planRepair()}>
            <Wrench className="h-3.5 w-3.5" />
            {t("issue.fixAllLinks")}
          </Button>
        )}
      </header>

      <div className="px-2 py-1.5">
        {issues.slice(0, 4).map((issue) => (
          <IssueRow key={`${issue.kind}:${issue.path}`} issue={issue} onRun={run} />
        ))}
        {issues.length > 4 && (
          <p className="px-1.5 py-1 text-[11.5px] text-text-dim">
            {t("issue.more", { count: issues.length - 4 })}
          </p>
        )}
      </div>
    </section>
  );
}
