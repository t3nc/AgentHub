import { PanelRight, RefreshCw, Search, X } from "lucide-react";
import { clsx } from "clsx";
import type { NavKey } from "../../lib/nav";
import { useHub } from "../../lib/useHub";
import { ThemeToggle } from "../../lib/theme";
import { shortPath } from "../../lib/format";
import { formatCombo } from "../../lib/commands";
import { useT } from "../../lib/i18n";
import { deltaTooltip, summarizeDelta } from "../../lib/fs-change";
import { Breadcrumbs } from "./Breadcrumbs";
import { PendingIssuesButton } from "./PendingIssues";

/**
 * 内嵌顶栏（在通高侧栏右侧）：模块标题 + 全局搜索 + 状态与动作。
 * 搜索框只做「跳到技能库并带上查询」，跨模块全文检索交给命令面板。
 */
export function Toolbar({
  active,
  onBack,
  onOpenPalette,
  onRefresh,
  loading,
  inspectorOpen,
  onToggleInspector,
  showInspector,
  search,
  onSearch,
  onOpenSettings,
  onShowAllIssues,
}: {
  active: NavKey;
  onBack: () => void;
  onOpenPalette: () => void;
  onRefresh: () => void;
  loading: boolean;
  inspectorOpen: boolean;
  onToggleInspector: () => void;
  /** 总览页自带右栏，不再重复提供检查器开关 */
  showInspector: boolean;
  search: string;
  onSearch: (next: string) => void;
  /** 待处理清单里「去设置」的落点 */
  onOpenSettings: () => void;
  /** 待处理清单里「查看全部」：跳到总览页的问题卡片 */
  onShowAllIssues: () => void;
}) {
  const { overview, fsChange, dismissFsChange, rescan } = useHub();
  const t = useT();
  const report = overview?.report;
  const fsSummary = summarizeDelta(fsChange);

  return (
    <header className="relative z-30 flex h-12 shrink-0 items-center gap-3 border-b border-line bg-ink-900 px-4">
      <div className="flex min-w-0 flex-1 items-center gap-3">
        <Breadcrumbs active={active} onBack={onBack} />

        <div className="hidden min-w-0 items-center gap-1.5 2xl:flex" title={overview?.data_root}>
          <span className="text-[11.5px] text-text-dim">{t("shell.dataRoot")}</span>
          <span className="mono max-w-[200px] truncate text-[11.5px] text-text-muted">
            {overview?.data_root ? shortPath(overview.data_root, 2) : "—"}
          </span>
        </div>
      </div>

      <div className="relative hidden h-8 w-[300px] shrink-0 items-center rounded-md border border-line bg-ink-950/50 pl-2.5 pr-1.5 transition-all focus-within:border-brand-500/50 focus-within:bg-ink-950 focus-within:ring-2 focus-within:ring-brand-500/15 lg:flex">
        <Search className="h-3.5 w-3.5 shrink-0 text-text-dim" />
        <input
          value={search}
          onChange={(event) => onSearch(event.target.value)}
          placeholder={t("shell.searchPlaceholder")}
          aria-label={t("shell.searchSkills")}
          className="h-full w-full border-0 bg-transparent px-1.5 text-[12px] text-text-strong outline-none placeholder:text-text-dim"
        />
        {search && (
          <button
            type="button"
            onClick={() => onSearch("")}
            aria-label={t("shell.clearSearch")}
            className="shrink-0 cursor-pointer rounded p-0.5 text-text-dim transition-colors hover:text-text-strong"
          >
            <X className="h-3.5 w-3.5" />
          </button>
        )}
      </div>

      <div className="flex shrink-0 items-center gap-1.5">
        {report && (
          <PendingIssuesButton
            onOpenSettings={onOpenSettings}
            onShowAll={onShowAllIssues}
          />
        )}

        {fsChange && (
          <span
            data-fs-change
            title={deltaTooltip(fsChange)}
            className="flex items-center gap-1.5 rounded-md border border-state-warn/40 bg-state-warn/10 px-2 py-1 text-[11.5px] text-state-warn"
          >
            {t("shell.fsChanged")}
            <span className="max-w-[180px] truncate opacity-70">{fsSummary.label}</span>
            <button
              type="button"
              aria-label={t("shell.rescanManaged")}
              className="cursor-pointer underline decoration-dotted"
              onClick={() => {
                void rescan();
              }}
            >
              {t("shell.rescanShort")}
            </button>
            <button
              type="button"
              aria-label={t("shell.dismissFsChange")}
              className="cursor-pointer text-state-warn/70 hover:text-state-warn"
              onClick={dismissFsChange}
            >
              <X className="h-3 w-3" />
            </button>
          </span>
        )}

        <span
          className="hidden items-center rounded-md border border-line px-1.5 py-1 text-[11px] text-text-dim md:flex"
          title={t("shell.platform")}
        >
          {overview?.platform ?? "—"}
        </span>

        <button
          type="button"
          onClick={onOpenPalette}
          data-open-palette
          title={t("shell.commandPalette", { combo: formatCombo("mod+k") })}
          className="flex cursor-pointer items-center gap-1.5 rounded-md border border-line bg-surface-2 px-2 py-1 text-[11.5px] text-text-muted transition-colors hover:border-line-strong hover:bg-surface-hover hover:text-text-strong"
        >
          <Search className="h-3.5 w-3.5" />
          <span className="mono hidden text-[11px] sm:inline">{formatCombo("mod+k")}</span>
        </button>

        <ThemeToggle />

        <button
          type="button"
          onClick={onRefresh}
          disabled={loading}
          className={clsx(
            "flex cursor-pointer items-center gap-1.5 rounded-md border px-2.5 py-1 text-[11.5px] font-medium transition-colors",
            loading
              ? "cursor-default border-line bg-surface-2 text-text-muted opacity-70"
              : "border-brand-500/60 bg-brand-500/90 text-brand-on hover:bg-brand-500"
          )}
        >
          <RefreshCw className={clsx("h-3.5 w-3.5", loading && "animate-spin")} />
          {loading ? t("shell.scanning") : t("shell.rescan")}
        </button>

        {showInspector && (
          <button
            type="button"
            onClick={onToggleInspector}
            aria-pressed={inspectorOpen}
            title={inspectorOpen ? t("shell.collapseInspector") : t("shell.expandInspector")}
            className={clsx(
              "cursor-pointer rounded-md border px-2 py-1 transition-colors",
              inspectorOpen
                ? "border-brand-500/40 bg-brand-500/10 text-brand-300"
                : "border-line bg-surface-2 text-text-muted hover:text-text-strong"
            )}
          >
            <PanelRight className="h-3.5 w-3.5" />
          </button>
        )}
      </div>
    </header>
  );
}
