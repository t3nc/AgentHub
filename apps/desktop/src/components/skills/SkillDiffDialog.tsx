import { useEffect, useMemo, useRef, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { clsx } from "clsx";
import { AlertTriangle, ArrowRightLeft, GitCompare, X } from "lucide-react";
import { useHub } from "../../lib/useHub";
import { Badge } from "../ui";
import { strategyLabel } from "../../lib/format";
import { useT } from "../../lib/i18n";
import { VERDICT_LABEL, buildSkillDiff, type Verdict } from "../../lib/skill-diff";
import type { LinkStrategyName } from "../../lib/types";

const VERDICT_TONE: Record<Verdict, "ok" | "warn" | "danger" | "dim"> = {
  same: "ok",
  diff: "warn",
  broken: "danger",
  single: "dim",
};

export function SkillDiffDialog({ name, onClose }: { name: string; onClose: () => void }) {
  const { overview, planDedupe, planRepair, setSkillStrategy } = useHub();
  const t = useT();
  const [baseline, setBaseline] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const panelRef = useRef<HTMLDivElement>(null);

  const diff = useMemo(
    () => (overview ? buildSkillDiff(overview.report.records, name) : null),
    [overview, name]
  );

  useEffect(() => {
    setBaseline(diff?.candidate ?? null);
  }, [diff?.candidate, name]);

  // Esc 关闭 + 关闭后把焦点还给触发元素（与命令面板一致）
  useEffect(() => {
    const trigger = document.activeElement;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onClose();
      }
    };
    window.addEventListener("keydown", onKey);
    panelRef.current?.focus();
    return () => {
      window.removeEventListener("keydown", onKey);
      if (trigger instanceof HTMLElement) trigger.focus();
    };
  }, [onClose]);

  if (!diff) return null;

  const selected = diff.members.find((member) => member.record.path === baseline) ?? null;
  const storeReady = diff.members.some((member) => member.record.source === "store");
  // 仓库里已有这份时，计划只是把其余副本改成指向仓库的链接，基线不再起作用——
  // 这仍然是个有意义的动作，所以按钮不能灰掉，只换文案。
  const canConverge = storeReady ? !busy : Boolean(selected?.usable) && !busy;
  const override = overview?.strategies.overrides[name] ?? null;
  const effective = override ?? overview?.strategies.default ?? null;

  const converge = async () => {
    if (!canConverge) return;
    setBusy(true);
    // 仓库已有版本时不传基线：后端会忽略它并在 note 里说明
    await planDedupe([name], storeReady ? null : baseline);
    setBusy(false);
    onClose();
  };

  return (
    <AnimatePresence>
      <motion.div
        key="skill-diff"
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0 }}
        transition={{ duration: 0.12 }}
        className="fixed inset-0 z-[65] flex items-start justify-center bg-[rgb(var(--scrim)/0.32)] px-6 pt-[9vh] dark:bg-[rgb(var(--scrim)/0.58)]"
        onMouseDown={(event) => {
          if (event.target === event.currentTarget) onClose();
        }}
      >
        <motion.div
          ref={panelRef}
          tabIndex={-1}
          role="dialog"
          aria-modal="true"
          aria-label={t("diff.aria", { name })}
          data-skill-diff
          initial={{ opacity: 0, y: -6 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: -4 }}
          transition={{ duration: 0.16, ease: "easeOut" }}
          className="flex max-h-[72vh] w-full flex-col overflow-hidden rounded-lg border border-line bg-ink-900 shadow-[var(--shadow-drawer)] outline-none md:w-[780px]"
        >
          <header className="flex items-start justify-between gap-3 border-b border-line px-3.5 py-2.5">
            <div className="min-w-0">
              <div className="flex items-center gap-2">
                <GitCompare className="h-3.5 w-3.5 shrink-0 text-text-dim" />
                <h2 className="truncate text-[13.5px] font-semibold text-text-strong">{name}</h2>
                <Badge tone={VERDICT_TONE[diff.verdict]}>
                  {t(VERDICT_LABEL[diff.verdict])}
                </Badge>
              </div>
              <p className="mt-0.5 text-[11.5px] text-text-dim" data-diff-summary>
                {diff.summary}
                {diff.readonlyCount > 0 &&
                  ` · ${t("diff.readonly", { count: diff.readonlyCount })}`}
                {diff.foreignCount > 0 &&
                  ` · ${t("diff.foreign", { count: diff.foreignCount })}`}
              </p>
            </div>
            <button
              type="button"
              onClick={onClose}
              aria-label={t("diff.close")}
              className="cursor-pointer rounded p-1 text-text-dim transition-colors hover:bg-surface-hover hover:text-text-strong"
            >
              <X className="h-4 w-4" />
            </button>
          </header>

          <div className="min-h-0 flex-1 overflow-auto">
            <table className="w-full border-collapse text-[11.5px]">
              <thead>
                <tr>
                  <th className="sticky left-0 z-10 min-w-[92px] border-b border-line bg-ink-900 px-3 py-2 text-left text-[11.5px] font-normal text-text-dim">
                    {t("diff.attrHeader")}
                  </th>
                  {diff.members.map((member) => {
                    const active = member.record.path === baseline;
                    return (
                      <th
                        key={member.record.path}
                        className={clsx(
                          "min-w-[180px] border-b border-line px-3 py-2 text-left align-top font-normal",
                          active && "bg-surface-2 shadow-[inset_0_-2px_0_0_rgb(var(--brand-500))]"
                        )}
                      >
                        <label className="flex cursor-pointer items-start gap-1.5">
                          <input
                            type="radio"
                            name={`baseline-${name}`}
                            value={member.record.path}
                            checked={active}
                            disabled={!member.usable}
                            title={member.reason ?? t("diff.baselineTitle")}
                            onChange={() => setBaseline(member.record.path)}
                            className={clsx(
                              "mt-0.5 h-3 w-3 accent-[rgb(var(--brand-500))]",
                              member.usable ? "cursor-pointer" : "cursor-not-allowed"
                            )}
                          />
                          <span className="min-w-0">
                            <span className="mono block truncate text-[11px] text-text-dim">
                              {member.label}
                            </span>
                            <span
                              className={clsx(
                                "mt-0.5 block truncate text-[11px]",
                                member.usable ? "text-text-muted" : "text-state-warn/80"
                              )}
                              title={member.reason}
                            >
                              {member.reason ?? member.sourceLabel}
                            </span>
                          </span>
                        </label>
                      </th>
                    );
                  })}
                </tr>
              </thead>
              <tbody>
                {diff.rows.map((row) => (
                  <tr key={row.key}>
                    <th
                      className={clsx(
                        "sticky left-0 z-10 border-b border-line/70 bg-ink-900 px-3 py-1.5 text-left text-[11.5px] font-normal",
                        row.same ? "text-text-dim" : "text-text-muted"
                      )}
                    >
                      {row.label}
                    </th>
                    {row.values.map((value, index) => {
                      // 路径与描述是长文本，截断会看不出差异，允许换行
                      const wrap = row.key === "path" || row.key === "description";
                      return (
                        <td
                          key={`${row.key}-${index}`}
                          className={clsx(
                            "border-b border-line/70 px-3 py-1.5 align-top",
                            row.same
                              ? "text-text-dim"
                              : index === row.outlier
                                ? "bg-state-warn/10 text-state-warn"
                                : "text-text-muted"
                          )}
                          title={value}
                        >
                          <span
                            className={clsx(
                              "mono block",
                              wrap ? "whitespace-normal break-all leading-snug" : "truncate"
                            )}
                          >
                            {value}
                          </span>
                        </td>
                      );
                    })}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          <footer className="flex shrink-0 items-center gap-2 border-t border-line px-3.5 py-2.5">
            <span className="min-w-0 flex-1 truncate text-[11.5px] text-text-dim" data-diff-baseline>
              {selected
                ? t("diff.baselineSelected", { name: selected.label })
                : t("diff.baselineNone")}
            </span>

            <select
              value={override ?? "auto"}
              onChange={(event) => {
                const value = event.target.value;
                void setSkillStrategy(name, value === "auto" ? null : (value as LinkStrategyName));
              }}
              className="cursor-pointer rounded border border-line bg-ink-950 px-1.5 py-1 text-[11px] text-text-muted outline-none"
              title={t("diff.effective", {
                name: effective ? strategyLabel(effective) : "—",
              })}
            >
              <option value="auto">{t("diff.followDefault")}</option>
              <option value="symlink">{t("common.strategy.symlink")}</option>
              <option value="junction">{t("common.strategy.junction")}</option>
              <option value="hardlink">{t("common.strategy.hardlink")}</option>
              <option value="copy">{t("common.strategy.copy")}</option>
            </select>

            {diff.brokenCount > 0 && (
              <button
                type="button"
                data-diff-repair
                onClick={() => {
                  void planRepair();
                  onClose();
                }}
                className="flex cursor-pointer items-center gap-1 rounded-md border border-state-danger/40 px-2 py-1 text-[11.5px] text-state-danger transition-colors hover:bg-state-danger/10"
              >
                <AlertTriangle className="h-3.5 w-3.5" />
                {t("diff.useRepair")}
              </button>
            )}

            <button
              type="button"
              data-diff-converge
              disabled={!canConverge}
              title={
                storeReady
                  ? t("diff.storeReadyNote")
                  : selected?.usable
                    ? t("diff.planWith", { name: selected.label })
                    : (selected?.reason ?? t("diff.pickBaseline"))
              }
              onClick={() => void converge()}
              className={clsx(
                "flex items-center gap-1 rounded-md px-2 py-1 text-[11.5px] transition-colors",
                canConverge
                  ? "cursor-pointer bg-brand-500 text-ink-950 hover:bg-brand-400"
                  : "cursor-not-allowed border border-line text-text-dim"
              )}
            >
              <ArrowRightLeft className="h-3.5 w-3.5" />
              {busy
                ? t("diff.building")
                : storeReady
                  ? t("diff.unifyToStore")
                  : t("diff.convergeToThis")}
            </button>
          </footer>
        </motion.div>
      </motion.div>
    </AnimatePresence>
  );
}
