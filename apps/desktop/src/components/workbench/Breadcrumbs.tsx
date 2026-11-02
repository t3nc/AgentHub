import { ChevronRight } from "lucide-react";
import { agentInfo, basename } from "../../lib/format";
import { useT } from "../../lib/i18n";
import { tr } from "../../lib/i18n/runtime";
import { navItem, type NavKey } from "../../lib/nav";
import { useSelection } from "../../lib/selection";

/** 选中对象 → 可读名字。技能存的是路径，取最后一段。 */
function objectLabel(kind: string, id: string): string {
  if (kind === "store") return tr("common.store");
  if (kind === "agent") return agentInfo(id).name;
  if (kind === "skill") return basename(id) || id;
  return id;
}

export function Breadcrumbs({
  active,
  onBack,
}: {
  active: NavKey;
  /** 点第一段：回模块根并清掉选中 */
  onBack: () => void;
}) {
  const { selection } = useSelection();
  const t = useT();
  const item = navItem(active);
  const label = t(item.label);

  return (
    <nav aria-label={t("shell.location")} className="flex min-w-0 items-center gap-1.5">
      <h1 className="shrink-0 text-[13.5px] font-semibold text-text-strong">
        {selection ? (
          <button
            type="button"
            onClick={onBack}
            data-breadcrumb-root
            className="cursor-pointer rounded-sm text-text-muted transition-colors hover:text-text-strong hover:underline hover:decoration-dotted hover:underline-offset-2"
          >
            {label}
          </button>
        ) : (
          label
        )}
      </h1>

      {selection ? (
        <>
          <ChevronRight className="h-3 w-3 shrink-0 text-text-dim" aria-hidden />
          <span
            data-breadcrumb-object
            className="min-w-0 truncate text-[12.5px] font-medium text-text-strong"
            title={selection.id}
          >
            {objectLabel(selection.kind, selection.id)}
          </span>
          <span className="hidden shrink-0 rounded border border-line px-1.5 py-0.5 text-[11px] text-text-dim md:inline">
            {selection.kind === "agent"
              ? t("shell.badge.agent")
              : selection.kind === "store"
                ? t("shell.badge.store")
                : t("shell.badge.skill")}
          </span>
        </>
      ) : (
        <span className="hidden truncate text-[11.5px] text-text-dim lg:inline">
          {t(item.hint)}
        </span>
      )}
    </nav>
  );
}
