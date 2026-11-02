import { AnimatePresence, motion } from "framer-motion";
import { AlertTriangle, ArrowRight, Info, X } from "lucide-react";
import { useHub } from "../lib/useHub";
import { useT } from "../lib/i18n";
import { shortPath } from "../lib/format";
import { Badge, Button } from "./ui";
import type { FileOp, RiskLevel } from "../lib/types";

function opTone(op: FileOp) {
  switch (op.action.op) {
    case "move_into_store":
      return "info" as const;
    case "remove_path":
      return "danger" as const;
    case "replace_with_link":
      return "warn" as const;
    case "create_link":
      return "ok" as const;
    default:
      return "brand" as const;
  }
}

function riskTone(level: RiskLevel) {
  return level === "danger" ? ("danger" as const) : level === "warn" ? ("warn" as const) : ("info" as const);
}

function OpRow({ op }: { op: FileOp }) {
  const a = op.action;
  return (
    <li className="panel-soft px-3 py-2 transition-colors hover:bg-surface-hover">
      <div className="flex items-center gap-2">
        <Badge tone={opTone(op)}>{op.label}</Badge>
        <span className="truncate text-[11px] text-text-dim">#{op.id + 1}</span>
      </div>
      <div className="mt-1.5 space-y-0.5 text-[11.5px]">
        {a.op === "move_into_store" && (
          <>
            <p className="mono truncate text-state-danger/90">- {shortPath(a.from, 4)}</p>
            <p className="mono truncate text-state-ok/90">+ {shortPath(a.to, 4)}</p>
          </>
        )}
        {a.op === "remove_path" && (
          <p className="mono truncate text-state-danger/90">- {shortPath(a.at, 4)}</p>
        )}
        {(a.op === "create_link" || a.op === "replace_with_link") && (
          <>
            <p className="mono truncate text-text-muted">{shortPath(a.at, 4)}</p>
            <p className="mono flex items-center gap-1 truncate text-state-ok/90">
              <ArrowRight className="h-3 w-3 shrink-0" />
              {shortPath(a.target, 4)}
              <span className="text-text-dim">({a.strategy})</span>
            </p>
          </>
        )}
        {a.op === "write_file" && (
          <>
            <p className="mono truncate text-text-muted">{shortPath(a.at, 4)}</p>
            <pre className="mt-1 max-h-24 overflow-auto rounded-md border border-line bg-ink-950/70 p-2 text-[11.5px] leading-relaxed text-text-muted">
              {a.content.slice(0, 400)}
              {a.content.length > 400 ? "\n…" : ""}
            </pre>
          </>
        )}
      </div>
    </li>
  );
}

export function PlanDrawer() {
  const { plan, setPlan, applyPlan, applying } = useHub();
  const t = useT();

  return (
    <AnimatePresence>
      {plan && (
        <motion.aside
          key="plan"
          initial={{ x: 460, opacity: 0 }}
          animate={{ x: 0, opacity: 1 }}
          exit={{ x: 460, opacity: 0 }}
          transition={{ type: "spring", stiffness: 320, damping: 32 }}
          className="fixed bottom-[30px] right-0 top-[58px] z-40 flex w-[450px] flex-col border-l border-line bg-ink-900/95 shadow-[var(--shadow-drawer)] backdrop-blur-2xl"
        >
          <div className="flex items-start justify-between gap-3 border-b border-line px-4 py-3">
            <div className="min-w-0">
              <div className="flex items-center gap-2">
                <Badge tone="brand">{t("plan.badge")}</Badge>
                <span className="mono truncate text-[11px] text-text-dim">{plan.id}</span>
              </div>
              <h3 className="mt-1 truncate text-[14px] font-medium text-text-strong">
                {plan.title}
              </h3>
            </div>
            <button
              type="button"
              onClick={() => setPlan(null)}
              className="cursor-pointer rounded-md p-1 text-text-dim transition-colors hover:bg-surface-hover hover:text-text-strong"
              aria-label={t("plan.close")}
            >
              <X className="h-4 w-4" />
            </button>
          </div>

          <div className="flex-1 space-y-3 overflow-y-auto px-4 py-3">
            {plan.risks.length > 0 && (
              <ul className="space-y-1.5">
                {plan.risks.map((risk, idx) => (
                  <li
                    key={idx}
                    className="flex items-start gap-2 rounded-lg border border-line bg-surface-2 px-2.5 py-1.5 text-[11.5px] leading-relaxed text-text-muted"
                  >
                    {risk.level === "info" ? (
                      <Info className="mt-[1px] h-3.5 w-3.5 shrink-0 text-state-info" />
                    ) : (
                      <AlertTriangle
                        className={`mt-[1px] h-3.5 w-3.5 shrink-0 ${
                          risk.level === "danger" ? "text-state-danger" : "text-state-warn"
                        }`}
                      />
                    )}
                    <span>{risk.message}</span>
                    <Badge tone={riskTone(risk.level)} className="ml-auto shrink-0">
                      {risk.level === "danger"
                        ? t("common.risk.danger")
                        : risk.level === "warn"
                          ? t("common.risk.warn")
                          : t("common.risk.info")}
                    </Badge>
                  </li>
                ))}
              </ul>
            )}

            <div>
              <p className="mb-1.5 text-[11.5px] text-text-dim">
                {t("plan.willRun", { count: plan.ops.length })}
              </p>
              <ul className="space-y-1.5">
                {plan.ops.map((op) => (
                  <OpRow key={op.id} op={op} />
                ))}
              </ul>
            </div>

            {plan.notes.length > 0 && (
              <ul className="space-y-1 border-t border-line pt-2">
                {plan.notes.map((note, idx) => (
                  <li key={idx} className="text-[11.5px] leading-relaxed text-text-dim">
                    · {note}
                  </li>
                ))}
              </ul>
            )}
          </div>

          <div className="flex items-center justify-end gap-2 border-t border-line px-4 py-3">
            <Button variant="ghost" onClick={() => setPlan(null)}>
              {t("common.cancel")}
            </Button>
            <Button
              variant="primary"
              onClick={() => void applyPlan()}
              disabled={applying || plan.ops.length === 0}
            >
              {applying
                ? t("plan.applying")
                : t("plan.apply", { count: plan.ops.length })}
            </Button>
          </div>
        </motion.aside>
      )}
    </AnimatePresence>
  );
}
