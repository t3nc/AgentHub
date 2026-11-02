import { ChevronsLeft, ChevronsRight } from "lucide-react";
import { clsx } from "clsx";
import { AgentHubMark } from "../brand/AgentHubMark";
import { visibleNav, type NavItem, type NavKey } from "../../lib/nav";
import { useT } from "../../lib/i18n";
import { tr } from "../../lib/i18n/runtime";
import { NAV_W, NAV_W_COLLAPSED } from "../../lib/layout";
import { bytes } from "../../lib/format";
import { useHub } from "../../lib/useHub";

export function NavRail({
  active,
  collapsed,
  onToggle,
  onNavigate,
}: {
  active: NavKey;
  collapsed: boolean;
  onToggle: () => void;
  onNavigate: (key: NavKey) => void;
}) {
  const { overview, features } = useHub();
  const t = useT();
  // 功能被禁用时整项从导航里消失（不是置灰）：用户不该看见一个点了没用的入口
  const items = visibleNav(features);
  /** 「设置」不参与 mod+1..8 的编号语义，单独放在主分组之后。 */
  const mainNav = items.filter((item) => item.key !== "settings");
  const settingsNav = items.filter((item) => item.key === "settings");
  const duplicates = overview?.report.duplicates.length ?? 0;
  const storeBytes = overview?.report.stats.store_bytes ?? 0;
  const agentBytes =
    overview?.report.records
      .filter((record) => record.agent)
      .reduce((sum, record) => sum + record.size_bytes, 0) ?? 0;
  const total = storeBytes + agentBytes;
  const storePct = total > 0 ? Math.max(3, Math.round((storeBytes / total) * 100)) : 50;

  const renderItem = ({ key, label, hint, icon: Icon }: NavItem) => {
    const isActive = key === active;
    const labelText = tr(label);
    const hintText = tr(hint);
    return (
      <button
        key={key}
        type="button"
        title={collapsed ? `${labelText} · ${hintText}` : hintText}
        aria-current={isActive ? "page" : undefined}
        onClick={() => onNavigate(key)}
        className={clsx(
          "group relative flex w-full cursor-pointer items-center rounded-md text-left transition-colors duration-150",
          collapsed ? "justify-center py-2.5" : "gap-2.5 px-2.5 py-[7px]",
          isActive
            ? "bg-nav-active text-nav-fg shadow-[inset_2px_0_0_0_rgb(var(--nav-accent))]"
            : "text-nav-dim hover:bg-nav-bg-2 hover:text-nav-fg"
        )}
      >
        <Icon
          className={clsx(
            "h-4 w-4 shrink-0",
            isActive ? "text-nav-accent" : "text-nav-dim group-hover:text-nav-fg"
          )}
        />
        {!collapsed && (
          <>
            <span className="flex-1 truncate text-[12.5px]">{labelText}</span>
            {key === "skills" && duplicates > 0 && (
              <span className="rounded border border-brass/50 bg-brass/15 px-1 font-mono text-[11px] text-brass">
                {duplicates}
              </span>
            )}
          </>
        )}
        {collapsed && key === "skills" && duplicates > 0 && (
          <span className="absolute right-1.5 top-1.5 h-1.5 w-1.5 rounded-full bg-brass" />
        )}
      </button>
    );
  };

  return (
    <aside
      data-nav-rail
      style={{ width: collapsed ? NAV_W_COLLAPSED : NAV_W }}
      className="flex h-full shrink-0 flex-col border-r border-nav-line bg-nav-bg text-nav-fg transition-[width] duration-150 ease-out"
    >
      <div
        className={clsx(
          "flex h-[52px] shrink-0 items-center gap-2.5 border-b border-nav-line",
          collapsed ? "justify-center px-0" : "px-3.5"
        )}
      >
        <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-nav-accent/40 bg-nav-accent/15 text-nav-accent">
          <AgentHubMark className="h-[18px] w-[18px]" />
        </span>
        {!collapsed && (
          <div className="min-w-0">
            <p className="truncate text-[12.5px] font-semibold tracking-wide text-nav-fg">AgentHub</p>
            <p className="truncate text-[11px] text-nav-dim">{t("shell.workbench")}</p>
          </div>
        )}
      </div>

      {!collapsed && (
        <p className="px-3.5 pb-1 pt-3.5 text-[11px] font-semibold uppercase tracking-[0.12em] text-nav-dim">
          {t("shell.workspace")}
        </p>
      )}

      <nav
        aria-label={t("shell.mainNav")}
        className={clsx("flex-1 space-y-0.5 overflow-y-auto py-1.5", collapsed ? "px-1.5" : "px-2")}
      >
        {mainNav.map(renderItem)}
        <div className={clsx("my-1.5 h-px bg-nav-line", collapsed && "mx-1")} role="presentation" />
        {settingsNav.map(renderItem)}
      </nav>

      {!collapsed && (
        <div className="shrink-0 border-t border-nav-line px-3.5 py-2.5">
          <div className="flex items-center justify-between text-[11.5px] text-nav-dim">
            <span>{t("shell.storeSpace")}</span>
            <span className="font-mono text-nav-fg">{bytes(total)}</span>
          </div>
          <div className="mt-1.5 flex h-1 overflow-hidden rounded-full bg-nav-line">
            <span className="h-full bg-nav-accent/80" style={{ width: `${storePct}%` }} />
            <span className="h-full flex-1 bg-brass/70" />
          </div>
          <p className="mt-1.5 text-[11px] leading-relaxed text-nav-dim">
            {t("shell.storeSplit", { store: bytes(storeBytes), copy: bytes(agentBytes) })}
          </p>
        </div>
      )}

      <div
        className={clsx(
          "flex shrink-0 items-center border-t border-nav-line",
          collapsed ? "flex-col gap-1.5 px-1.5 py-2" : "justify-between px-3 py-2"
        )}
      >
        {collapsed ? (
          <span
            className="h-1.5 w-1.5 rounded-full bg-nav-accent"
            title={overview?.demo ? t("shell.demoData") : t("shell.running")}
          />
        ) : (
          <span className="flex min-w-0 items-center gap-1.5 text-[11.5px] text-nav-dim">
            <span className="h-1.5 w-1.5 shrink-0 rounded-full bg-nav-accent" />
            <span className="truncate">
              {overview?.demo ? t("shell.demoData") : t("shell.running")}
            </span>
          </span>
        )}
        <button
          type="button"
          onClick={onToggle}
          title={collapsed ? t("shell.expandNav") : t("shell.collapseToIcons")}
          aria-label={collapsed ? t("shell.expandNav") : t("shell.collapseNav")}
          className="flex cursor-pointer items-center justify-center rounded-md p-1 text-nav-dim transition-colors hover:bg-nav-bg-2 hover:text-nav-fg"
        >
          {collapsed ? (
            <ChevronsRight className="h-3.5 w-3.5" />
          ) : (
            <ChevronsLeft className="h-3.5 w-3.5" />
          )}
        </button>
      </div>
    </aside>
  );
}
