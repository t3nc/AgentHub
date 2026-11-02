import { clsx } from "clsx";
import { ClipboardList, RotateCcw } from "lucide-react";
import { useHub } from "../../lib/useHub";
import { useT } from "../../lib/i18n";
import { bytes, shortPath, timeAgo } from "../../lib/format";

export function StatusBar({
  onRollback,
  onOpenChanges,
}: {
  onRollback: (batch: string) => void;
  onOpenChanges: (batch?: string | null) => void;
}) {
  const { overview, plan } = useHub();
  const t = useT();
  const report = overview?.report;
  const last = overview?.batches[0];

  return (
    <footer className="flex h-[30px] shrink-0 items-center gap-3 border-t border-line bg-ink-900 px-4 text-[11.5px] text-text-muted">
      <span className="flex items-center gap-1.5">
        <span className="h-1.5 w-1.5 rounded-full bg-state-ok" />
        {t("status.scan", { ms: report?.duration_ms ?? 0 })}
      </span>
      <span>{t("status.skills", { count: report?.stats.total ?? 0 })}</span>
      <span>{t("status.shared", { count: report?.stats.shared_skills ?? 0 })}</span>
      <span className="text-state-warn">
        {t("status.wasted", { bytes: bytes(report?.stats.wasted_bytes ?? 0) })}
      </span>

      {plan ? (
        <button
          type="button"
          data-batch-context
          onClick={() => onOpenChanges(null)}
          title={t("status.planTitle", { title: plan.title, count: plan.ops.length })}
          className="flex cursor-pointer items-center gap-1 rounded border border-brass/50 px-1.5 py-0.5 text-[11px] text-brass transition-colors hover:bg-brass/10"
        >
          <ClipboardList className="h-3 w-3" />
          {t("status.planChip", { count: plan.ops.length })}
        </button>
      ) : (
        last && (
          <button
            type="button"
            data-batch-context
            onClick={() => onOpenChanges(last.batch_id)}
            title={`${last.title} · ${timeAgo(last.created_at)}`}
            className={clsx(
              "flex cursor-pointer items-center gap-1 rounded border px-1.5 py-0.5 text-[11px] transition-colors",
              last.undone
                ? "border-line text-text-dim hover:bg-surface-hover"
                : "border-line text-text-muted hover:bg-surface-hover hover:text-text-strong"
            )}
          >
            <ClipboardList className="h-3 w-3" />
            {t("status.batchChip", {
              id: last.batch_id.slice(-6),
              time: last.undone ? t("status.rolledBack") : timeAgo(last.created_at),
            })}
          </button>
        )
      )}

      <span
        className="mono ml-auto max-w-[280px] truncate text-text-dim"
        title={report?.store_root}
      >
        {report?.store_root ? shortPath(report.store_root, 2) : t("common.dash")}
      </span>

      {last && (
        <button
          type="button"
          onClick={() => onRollback(last.batch_id)}
          disabled={last.undone}
          title={
            last.undone
              ? t("status.rolledBackTip")
              : t("status.rollbackBatch", { title: last.title })
          }
          className={clsx(
            "flex items-center gap-1 rounded px-1.5 py-0.5 transition-colors",
            last.undone
              ? "cursor-not-allowed text-text-dim"
              : "cursor-pointer text-text-muted hover:bg-surface-hover hover:text-text-strong"
          )}
        >
          <RotateCcw className="h-3 w-3" />
          {last.undone
            ? t("status.rolledBack")
            : t("status.rollbackId", { id: last.batch_id.slice(-6) })}
        </button>
      )}
    </footer>
  );
}
