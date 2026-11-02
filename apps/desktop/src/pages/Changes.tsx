import { useEffect, useRef, useState } from "react";
import { clsx } from "clsx";
import { AlertTriangle, History, Info, RotateCcw } from "lucide-react";
import { useHub } from "../lib/useHub";
import { useContextMenu, useMenuEnv } from "../lib/context-menu";
import { composeMenu } from "../lib/menu-builders/common";
import { batchRowMenu, changesPageMenu } from "../lib/menu-builders/changes";
import { ConfirmDialog } from "../components/workbench/ConfirmDialog";
import { bytes, timeAgo } from "../lib/format";
import { useT } from "../lib/i18n";
import type { MessageKey } from "../lib/i18n/messages";
import { tr } from "../lib/i18n/runtime";
import { Badge, Button, Empty, Panel } from "../components/ui";

/**
 * 批次类型：新数据存 slug，老数据存的是当时的自然语言标签。
 *
 * slug 认不出来（老批次）就直接显示原值——历史记录保持写入时的语言，
 * 不硬翻成当前语言，免得「这条批次到底做了什么」变得可疑。
 */
const BATCH_KIND_KEYS: Record<string, MessageKey> = {
  dedupe: "common.kind.dedupe",
  adopt: "common.kind.adopt",
  repair: "common.kind.repair",
  mcp_write: "common.kind.mcpWrite",
  instruction: "common.kind.instruction",
  project_init: "common.kind.projectInit",
  provider: "common.kind.provider",
  delete: "common.kind.delete",
  market_install: "common.kind.marketInstall",
  rollback: "common.kind.rollback",
};

function batchKindLabel(kind: string): string {
  const key = BATCH_KIND_KEYS[kind];
  return key ? tr(key) : kind;
}

export function Changes({ focusBatch }: { focusBatch?: string | null }) {
  const { overview, plan, setPlan, applyPlan, applying, rollback, planDedupe, planRepair, notify } =
    useHub();
  const t = useT();
  const menuEnv = useMenuEnv();
  const contextMenu = useContextMenu();
  /** 右键「回滚该批次」先落一个待确认的批次号 */
  const [pendingRollback, setPendingRollback] = useState<string | null>(null);
  const focusedRef = useRef<HTMLTableRowElement | null>(null);

  // 从状态栏批次胶囊跳过来时，把目标批次滚到可见位置
  useEffect(() => {
    if (!focusBatch) return;
    focusedRef.current?.scrollIntoView({ block: "nearest", behavior: "smooth" });
  }, [focusBatch, overview?.batches.length]);

  if (!overview) return null;

  const batches = overview.batches;
  const backupBytes = overview.report.stats.wasted_bytes;

  const openBatchMenu = (event: React.MouseEvent, batch: (typeof batches)[number]) => {
    contextMenu.open(
      event,
      composeMenu(
        batchRowMenu(batch, {
          openBatch: () => {
            window.location.hash = `#changes?batch=${encodeURIComponent(batch.batch_id)}`;
          },
          rollback: () => void rollback(batch.batch_id),
          confirmRollback: () => setPendingRollback(batch.batch_id),
          notify,
        }),
        menuEnv,
        "changes"
      )
    );
  };

  const openPageMenu = (event: React.MouseEvent) => {
    contextMenu.open(
      event,
      composeMenu(
        changesPageMenu({
          hasPlan: Boolean(plan),
          planOps: plan?.ops.length ?? 0,
          applying,
          apply: () => void applyPlan(),
          discard: () => setPlan(null),
        }),
        menuEnv,
        "changes"
      )
    );
  };

  return (
    <>
    <div className="space-y-3.5 animate-fade-up" onContextMenu={openPageMenu}>
      <div className="grid grid-cols-[1.4fr_1fr] gap-3.5">
        <Panel
          title={plan ? plan.title : t("changes.currentPlan")}
          subtitle={
            plan
              ? t("changes.planSubtitle", { count: plan.ops.length })
              : t("changes.noPlan")
          }
          actions={
            plan ? (
              <>
                <Button size="sm" variant="ghost" onClick={() => setPlan(null)}>
                  {t("settings.discard")}
                </Button>
                <Button
                  size="sm"
                  variant="primary"
                  disabled={applying || plan.ops.length === 0}
                  onClick={() => void applyPlan()}
                >
                  {applying ? t("plan.applying") : t("changes.applyNow")}
                </Button>
              </>
            ) : (
              <>
                <Button size="sm" variant="outline" onClick={() => void planDedupe()}>
                  {t("inspector.planDedupe")}
                </Button>
                <Button size="sm" variant="ghost" onClick={() => void planRepair()}>
                  {t("dash.repairBroken")}
                </Button>
              </>
            )
          }
        >
          {!plan ? (
            <Empty
              title={t("changes.emptyTitle")}
              hint={t("changes.emptyHint")}
              action={
                <Button size="sm" variant="primary" onClick={() => void planDedupe()}>
                  {t("changes.planNow")}
                </Button>
              }
            />
          ) : (
            <div className="space-y-2.5">
              <div className="grid grid-cols-3 gap-2">
                <div className="panel-soft px-2.5 py-2">
                  <p className="text-[11.5px] text-text-dim">{t("changes.opCount")}</p>
                  <p className="mono text-[16px] text-text-strong">{plan.ops.length}</p>
                </div>
                <div className="panel-soft px-2.5 py-2">
                  <p className="text-[11.5px] text-text-dim">{t("changes.riskCount")}</p>
                  <p className="mono text-[16px] text-text-strong">{plan.risks.length}</p>
                </div>
                <div className="panel-soft px-2.5 py-2">
                  <p className="text-[11.5px] text-text-dim">{t("changes.planId")}</p>
                  <p className="mono truncate text-[11px] text-text-muted">{plan.id}</p>
                </div>
              </div>

              <ul className="max-h-[320px] space-y-1.5 overflow-y-auto pr-1">
                {plan.ops.map((op) => (
                  <li
                    key={op.id}
                    className="rounded-lg border border-line bg-surface-2 px-2.5 py-2"
                  >
                    <div className="flex items-center gap-2">
                      <Badge
                        tone={
                          op.action.op === "remove_path"
                            ? "danger"
                            : op.action.op === "create_link"
                              ? "ok"
                              : op.action.op === "move_into_store"
                                ? "info"
                                : "warn"
                        }
                      >
                        {op.label}
                      </Badge>
                      <span className="mono min-w-0 flex-1 truncate text-[11px] text-text-muted">
                        {op.action.op === "move_into_store"
                          ? `${op.action.from} → ${op.action.to}`
                          : op.action.op === "write_file"
                            ? op.action.at
                            : op.action.op === "remove_path"
                              ? op.action.at
                              : `${op.action.at} → ${op.action.target}`}
                      </span>
                    </div>
                  </li>
                ))}
              </ul>

              {plan.risks.length > 0 && (
                <ul className="space-y-1">
                  {plan.risks.map((risk, idx) => (
                    <li key={idx} className="flex items-start gap-1.5 text-[11px] leading-relaxed text-text-muted">
                      {risk.level === "info" ? (
                        <Info className="mt-[1px] h-3 w-3 shrink-0 text-state-info" />
                      ) : (
                        <AlertTriangle
                          className={`mt-[1px] h-3 w-3 shrink-0 ${
                            risk.level === "danger" ? "text-state-danger" : "text-state-warn"
                          }`}
                        />
                      )}
                      {risk.message}
                    </li>
                  ))}
                </ul>
              )}
            </div>
          )}
        </Panel>

        <Panel
          title={t("changes.protection.title")}
          subtitle={t("changes.protection.subtitle")}
        >
          <ul className="space-y-2 text-[11.5px] leading-relaxed text-text-muted">
            <li className="flex gap-2">
              <Badge tone="brand">1</Badge>
              {t("changes.protection.p1")}
            </li>
            <li className="flex gap-2">
              <Badge tone="brand">2</Badge>
              {t("changes.protection.p2")}
            </li>
            <li className="flex gap-2">
              <Badge tone="brand">3</Badge>
              {t("changes.protection.p3")}
            </li>
            <li className="flex gap-2">
              <Badge tone="brand">4</Badge>
              {t("changes.protection.p4")}
            </li>
          </ul>
          <div className="mt-2.5 rounded-lg border border-line bg-surface-2 px-2.5 py-2 text-[11.5px] leading-relaxed text-text-dim">
            {t("changes.protection.space", { bytes: bytes(backupBytes) })}
          </div>
        </Panel>
      </div>

      <Panel
        title={t("changes.history.title")}
        subtitle={t("changes.history.subtitle")}
        actions={<History className="h-3.5 w-3.5 text-text-dim" />}
      >
        {batches.length === 0 ? (
          <Empty
            title={t("changes.history.emptyTitle")}
            hint={t("changes.history.emptyHint")}
          />
        ) : (
          <table className="w-full text-[12px]">
            <thead>
              <tr className="border-b border-line text-left text-[11px] text-text-dim">
                <th className="py-1.5 font-normal">{t("changes.col.batch")}</th>
                <th className="py-1.5 font-normal">{t("changes.col.kind")}</th>
                <th className="py-1.5 font-normal">{t("changes.col.note")}</th>
                <th className="py-1.5 text-right font-normal">{t("changes.col.ops")}</th>
                <th className="py-1.5 text-right font-normal">{t("changes.col.time")}</th>
                <th className="w-[92px] py-1.5 text-right font-normal">
                  {t("changes.col.status")}
                </th>
              </tr>
            </thead>
            <tbody>
              {batches.map((batch) => (
                <tr
                  key={batch.batch_id}
                  data-batch={batch.batch_id}
                  data-ctx="batch-row"
                  onContextMenu={(event) => openBatchMenu(event, batch)}
                  ref={batch.batch_id === focusBatch ? focusedRef : undefined}
                  className={clsx(
                    "border-b border-line",
                    batch.batch_id === focusBatch &&
                      "bg-brand-500/[0.06] outline outline-1 -outline-offset-1 outline-brand-500/40"
                  )}
                >
                  <td className="mono py-1.5 pl-2 text-[11.5px] text-text-dim">
                    {batch.batch_id === focusBatch && (
                      <span className="mr-1.5 inline-block h-2.5 w-0.5 rounded-full bg-brand-500 align-middle" />
                    )}
                    {batch.batch_id}
                  </td>
                  <td className="py-1.5">
                    <Badge
                      tone={
                        batch.kind === "dedupe"
                          ? "brand"
                          : batch.kind === "delete"
                            ? "warn"
                            : "dim"
                      }
                    >
                      {batchKindLabel(batch.kind)}
                    </Badge>
                  </td>
                  <td className="py-1.5 text-text-muted">{batch.title}</td>
                  <td className="mono py-1.5 text-right text-text-muted">{batch.op_count}</td>
                  <td className="py-1.5 text-right text-[11px] text-text-dim">{timeAgo(batch.created_at)}</td>
                  <td className="py-1.5 text-right">
                    {batch.undone ? (
                      <Badge tone="dim">{t("changes.rolledBack")}</Badge>
                    ) : (
                      <Button size="sm" variant="ghost" onClick={() => void rollback(batch.batch_id)}>
                        <RotateCcw className="h-3 w-3" />
                        {t("changes.rollback")}
                      </Button>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Panel>
    </div>

    <ConfirmDialog
      open={pendingRollback !== null}
      title={t("changes.rollbackTitle")}
      body={
        pendingRollback ? t("changes.rollbackBody", { batch: pendingRollback }) : ""
      }
      confirmLabel={t("changes.rollback")}
      onCancel={() => setPendingRollback(null)}
      onConfirm={() => {
        const batchId = pendingRollback;
        setPendingRollback(null);
        if (batchId) void rollback(batchId);
      }}
    />
    </>
  );
}
