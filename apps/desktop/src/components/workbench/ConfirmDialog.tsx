import { AnimatePresence, motion } from "framer-motion";
import { AlertTriangle, X } from "lucide-react";
import { Button } from "../ui";
import { useT } from "../../lib/i18n";

/**
 * 轻量二次确认。
 *
 * 只用于「右键菜单里点了危险项」这一类场景：删除技能走的是既有
 * `DeleteSkillsDialog`（要列清单），这里负责回滚批次、移出受管这种一句话能说清的确认。
 */
export function ConfirmDialog({
  open,
  title,
  body,
  confirmLabel,
  danger = true,
  onCancel,
  onConfirm,
}: {
  open: boolean;
  title: string;
  body: string;
  confirmLabel: string;
  danger?: boolean;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  const t = useT();
  return (
    <AnimatePresence>
      {open && (
        <motion.div
          key="confirm"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.1 }}
          className="fixed inset-0 z-[68] flex items-start justify-center bg-[rgb(var(--scrim)/0.32)] px-6 pt-[18vh] dark:bg-[rgb(var(--scrim)/0.62)]"
          onMouseDown={(event) => {
            if (event.target === event.currentTarget) onCancel();
          }}
        >
          <motion.div
            role="alertdialog"
            aria-modal="true"
            aria-label={title}
            data-confirm-dialog
            initial={{ opacity: 0, y: -6 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -4 }}
            transition={{ duration: 0.14, ease: "easeOut" }}
            className="w-[420px] overflow-hidden rounded-lg border border-line bg-ink-900 shadow-[var(--shadow-drawer)]"
          >
            <header className="flex items-start gap-2.5 border-b border-line px-4 py-3">
              <span className="mt-0.5 flex h-7 w-7 shrink-0 items-center justify-center rounded-md border border-state-warn/30 bg-state-warn/10 text-state-warn">
                <AlertTriangle className="h-3.5 w-3.5" />
              </span>
              <div className="min-w-0 flex-1">
                <h2 className="text-[13.5px] font-medium text-text-strong">{title}</h2>
                <p className="mt-0.5 text-[11px] leading-relaxed text-text-dim">{body}</p>
              </div>
              <button
                type="button"
                onClick={onCancel}
                aria-label={t("common.close")}
                className="cursor-pointer rounded p-1 text-text-dim transition-colors hover:bg-surface-hover hover:text-text-strong"
              >
                <X className="h-3.5 w-3.5" />
              </button>
            </header>
            <footer className="flex items-center justify-end gap-1.5 px-4 py-2.5">
              <Button size="sm" variant="ghost" onClick={onCancel}>
                {t("common.cancel")}
              </Button>
              <Button size="sm" variant={danger ? "danger" : "primary"} onClick={onConfirm}>
                {confirmLabel}
              </Button>
            </footer>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
